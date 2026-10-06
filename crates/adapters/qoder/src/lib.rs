use async_trait::async_trait;
use localmux_adapter_sdk::{
    cli::{raw_output, stream_json},
    collect_events, text_stream, Adapter, AdapterError, EventStream, NormalizedRequest,
    NormalizedResponse, NormalizedStream, SessionEventStream, SessionStream,
};
use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

pub struct QoderAdapter {
    executable: PathBuf,
    id: &'static str,
}
impl QoderAdapter {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            id: "qoder",
        }
    }
    pub fn new_cn(executable: PathBuf) -> Self {
        Self {
            executable,
            id: "qoder-cn",
        }
    }
    async fn capabilities(&self) -> Result<localmux_domain::ChatCapabilities, AdapterError> {
        let help = localmux_adapter_sdk::cli::raw_output(&self.executable, &["--help"]).await?;
        let help = String::from_utf8_lossy(&help);
        let version =
            localmux_adapter_sdk::cli::raw_output(&self.executable, &["--version"]).await?;
        let version = String::from_utf8_lossy(&version).trim().to_owned();
        if !help.contains("--setting-sources")
            || !help.contains("--tools")
            || !help.contains("--strict-mcp-config")
        {
            return Err(AdapterError::Unsupported);
        }
        let mut chat = localmux_domain::ChatCapabilities::single_turn();
        if help.contains("--system-prompt") {
            chat.system = localmux_domain::Support::Supported;
        }
        chat.source = "公开 CLI 参数与离线映射；真实效果未验证".into();
        chat.describe(Some(version), "headless_stream_json");
        for name in [
            "temperature",
            "top_p",
            "max_tokens",
            "max_completion_tokens",
        ] {
            let detail = chat.details.get_mut(name).unwrap();
            detail.limiting_layer = Some("public_interface".into());
            detail.reason_code = Some("public_control_unverified".into());
        }
        if help.contains("--max-output-tokens") {
            chat.public_candidate("max_tokens", "unknown_constraints_and_counting_basis");
            chat.public_candidate(
                "max_completion_tokens",
                "unknown_constraints_and_counting_basis",
            );
        }
        chat.details.get_mut("developer").unwrap().limiting_layer = Some("public_interface".into());
        chat.details.get_mut("developer").unwrap().reason_code =
            Some("role_priority_unmapped".into());
        for name in ["usage", "finish_reason"] {
            let detail = chat.details.get_mut(name).unwrap();
            detail.effective_support = localmux_domain::Support::Unknown;
            detail.adapter_state = "NOT_IMPLEMENTED".into();
            detail.reason_code = Some("public_result_metadata_unverified".into());
            detail.limiting_layer = Some("adapter".into());
        }
        Ok(chat)
    }
    pub async fn invoke_with_session(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<NormalizedResponse, AdapterError> {
        let (stream, session_id) = self.stream_with_session(request, session).await?;
        let mut response = collect_events(stream).await?;
        response.provider_session_id = session_id
            .lock()
            .await
            .clone()
            .or_else(|| session.map(str::to_owned));
        Ok(response)
    }

    async fn stream_with_session(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<(EventStream, Arc<Mutex<Option<String>>>), AdapterError> {
        let instructions = request.text_instructions(false)?;
        let capabilities = self.capabilities().await?;
        request.parameters.validate(&capabilities)?;
        if instructions.system.is_some()
            && capabilities.system != localmux_domain::Support::Supported
        {
            return Err(AdapterError::UnsupportedMessages);
        }
        let model = request
            .model
            .strip_prefix(&format!("{}/", self.id))
            .ok_or(AdapterError::ModelUnavailable)?;
        if !valid_model(model) {
            return Err(AdapterError::ModelUnavailable);
        }
        let mut args = [
            "-p",
            "--output-format",
            "stream-json",
            "--permission-mode",
            "dont_ask",
            "--tools",
            "",
            "--strict-mcp-config",
            "--max-turns",
            "1",
            "--model",
            model,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        args.extend(["--setting-sources".into(), "".into()]);
        if let Some(system) = instructions.system {
            args.extend(["--system-prompt".into(), system]);
        }
        if let Some(id) = session {
            args.extend(["--session-id".to_owned(), id.to_owned()]);
        }
        stream_json(
            &self.executable,
            &args,
            request.prompt,
            parse_delta,
            map_error,
        )
        .await
    }
}
#[async_trait]
impl Adapter for QoderAdapter {
    fn id(&self) -> &'static str {
        self.id
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Agent
    }
    async fn health(&self) -> HealthStatus {
        if self.executable.is_file() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Offline
        }
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        let mut models = parse_models_output(
            &raw_output(
                &self.executable,
                &["--list-models", "--output-format", "json"],
            )
            .await?,
            self.id,
        )?;
        let chat = self.capabilities().await?;
        for model in &mut models {
            model.chat = chat.clone();
        }
        Ok(models)
    }

    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        self.invoke_with_session(request, None).await
    }
    async fn invoke_stream(
        &self,
        request: NormalizedRequest,
    ) -> Result<NormalizedStream, AdapterError> {
        self.stream_with_session(request, None)
            .await
            .map(|(stream, _)| text_stream(stream))
    }
    async fn resume(
        &self,
        request: NormalizedRequest,
        provider_session_id: &str,
    ) -> Result<NormalizedResponse, AdapterError> {
        self.invoke_with_session(request, Some(provider_session_id))
            .await
    }
    async fn resume_stream(
        &self,
        request: NormalizedRequest,
        provider_session_id: &str,
    ) -> Result<NormalizedStream, AdapterError> {
        self.stream_with_session(request, Some(provider_session_id))
            .await
            .map(|(stream, _)| text_stream(stream))
    }
    async fn invoke_events(&self, request: NormalizedRequest) -> Result<EventStream, AdapterError> {
        self.stream_with_session(request, None)
            .await
            .map(|(stream, _)| stream)
    }
    async fn invoke_session_events(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<SessionEventStream, AdapterError> {
        let (stream, provider_session_id) = self.stream_with_session(request, session).await?;
        Ok(SessionEventStream {
            stream,
            provider_session_id,
        })
    }
    async fn invoke_session_stream(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<SessionStream, AdapterError> {
        let (stream, provider_session_id) = self.stream_with_session(request, session).await?;
        Ok(SessionStream {
            stream: text_stream(stream),
            provider_session_id,
        })
    }
}
fn valid_model(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
}
fn parse_models(
    value: &Value,
    provider: &'static str,
) -> Result<Vec<ModelDescriptor>, AdapterError> {
    let data = value
        .as_array()
        .or_else(|| value["models"].as_array())
        .or_else(|| value["data"].as_array())
        .ok_or(AdapterError::Unsupported)?;
    let models: Vec<_> = data
        .iter()
        .filter_map(|item| {
            if item["isEnabled"] == false || item["is_enabled"] == false {
                return None;
            }
            let id = item
                .as_str()
                .or_else(|| item["id"].as_str())
                .or_else(|| item["model"].as_str())
                .or_else(|| item["value"].as_str())?;
            if !valid_model(id) {
                return None;
            }
            Some(ModelDescriptor {
                id: format!("{provider}/{id}"),
                provider_id: provider.into(),
                connection_id: None,
                cost_mode: CostMode::Unknown,
                supports_streaming: true,
                supports_session: true,
                chat: localmux_domain::ChatCapabilities::single_turn(),
            })
        })
        .collect();
    if models.is_empty() {
        Err(AdapterError::Unsupported)
    } else {
        Ok(models)
    }
}
fn parse_models_output(
    output: &[u8],
    provider: &'static str,
) -> Result<Vec<ModelDescriptor>, AdapterError> {
    if let Ok(value) = serde_json::from_slice::<Value>(output) {
        return parse_models(&value, provider);
    }
    let text = std::str::from_utf8(output).map_err(|_| AdapterError::Unsupported)?;
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    if lines.next() != Some("MODEL") {
        return Err(AdapterError::Unsupported);
    }
    let models: Vec<_> = lines
        .filter(|id| valid_model(id))
        .map(|id| ModelDescriptor {
            id: format!("{provider}/{id}"),
            provider_id: provider.into(),
            connection_id: None,
            cost_mode: CostMode::Unknown,
            supports_streaming: true,
            supports_session: true,
            chat: localmux_domain::ChatCapabilities::single_turn(),
        })
        .collect();
    if models.is_empty() {
        Err(AdapterError::Unsupported)
    } else {
        Ok(models)
    }
}
fn parse_delta(value: &Value) -> Option<String> {
    if value["type"] == "stream_event" && value["event"]["delta"]["type"] == "text_delta" {
        value["event"]["delta"]["text"].as_str().map(str::to_owned)
    } else {
        None
    }
}
fn map_error(value: &Value) -> AdapterError {
    let message = value["message"].as_str().unwrap_or("").to_ascii_lowercase();
    if message.contains("auth") || message.contains("login") {
        AdapterError::AuthRequired
    } else if message.contains("rate") {
        AdapterError::RateLimited
    } else if message.contains("quota") || message.contains("credit") {
        AdapterError::QuotaExhausted
    } else {
        AdapterError::ProcessFailed
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_catalog_entries_are_models() {
        let data = serde_json::json!({"models":[{"id":"auto"},{"value":"efficient"},{"value":"disabled","isEnabled":false},{"name":"not-an-id"}]});
        let models = parse_models(&data, "qoder").unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].cost_mode, CostMode::Unknown);
        assert!(parse_models(&serde_json::json!({"unexpected":"shape"}), "qoder").is_err());
    }
    #[test]
    fn installed_cli_table_is_a_reported_catalog() {
        let models = parse_models_output(b"MODEL\nAuto\nQwen3.8-Max\n", "qoder").unwrap();
        assert_eq!(
            models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["qoder/Auto", "qoder/Qwen3.8-Max"]
        );
        assert!(parse_models_output(b"not a model catalog\nAuto\n", "qoder").is_err());
        assert_eq!(
            parse_models_output(b"MODEL\nAuto\n", "qoder-cn").unwrap()[0].id,
            "qoder-cn/Auto"
        );
    }
    #[test]
    fn only_text_deltas_are_exposed() {
        assert_eq!(
            parse_delta(
                &serde_json::json!({"type":"stream_event","event":{"delta":{"type":"text_delta","text":"hi"}}})
            ),
            Some("hi".into())
        );
        assert_eq!(
            parse_delta(
                &serde_json::json!({"type":"stream_event","event":{"delta":{"type":"thinking_delta","thinking":"private"}}})
            ),
            None
        );
    }
}
