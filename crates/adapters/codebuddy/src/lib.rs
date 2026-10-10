use async_trait::async_trait;
use localmux_adapter_sdk::{
    cli::stream_json_command, collect_events, text_stream, Adapter, AdapterError, EventStream,
    NormalizedRequest, NormalizedResponse, NormalizedStream, SessionEventStream, SessionStream,
};
use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
use serde_json::Value;
use std::{env, path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{process::Command, sync::Mutex};

pub struct CodeBuddyAdapter {
    executable: PathBuf,
}
impl CodeBuddyAdapter {
    pub fn new(executable: PathBuf) -> Self {
        Self { executable }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        if let Some(node_dir) = newer_node_dir() {
            let mut paths = vec![node_dir];
            paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
            if let Ok(path) = env::join_paths(paths) {
                command.env("PATH", path);
            }
        }
        command
    }
    async fn catalog_output(&self, args: &[&str]) -> Result<Vec<u8>, AdapterError> {
        // Shared bounded output also isolates cwd and owns the cancellation process group.
        let output = localmux_adapter_sdk::cli::command_output(
            self.command(),
            args,
            Duration::from_secs(15),
        )
        .await?;
        if !output.status.success() {
            return Err(map_error(
                &serde_json::json!({"message": String::from_utf8_lossy(&output.stderr)}),
            ));
        }
        Ok(output.stdout)
    }
    async fn capabilities(&self) -> Result<localmux_domain::ChatCapabilities, AdapterError> {
        let help = self.catalog_output(&["--help"]).await?;
        let help = String::from_utf8_lossy(&help);
        let version = self.catalog_output(&["--version"]).await?;
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
        if help.contains("--json-schema") {
            chat.public_candidate("json_schema", "guarantee_unverified");
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
            .strip_prefix("codebuddy/")
            .ok_or(AdapterError::ModelUnavailable)?;
        if !valid_model(model) {
            return Err(AdapterError::ModelUnavailable);
        }
        let mut args = [
            "-p",
            "--output-format",
            "stream-json",
            "--include-partial-messages",
            "--permission-mode",
            "dontAsk",
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
            args.extend(["--resume".to_owned(), id.to_owned()]);
        }
        stream_json_command(
            self.command(),
            &args,
            request.prompt,
            parse_delta,
            map_error,
        )
        .await
    }
}
#[async_trait]
impl Adapter for CodeBuddyAdapter {
    fn id(&self) -> &'static str {
        "codebuddy"
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
        if !self.executable.is_file() {
            return Err(AdapterError::ProviderUnavailable);
        }
        let output = self.catalog_output(&["config", "get", "model"]).await?;
        let models = match parse_selected_model(&output) {
            Ok(model) => vec![model],
            Err(AdapterError::ModelUnavailable) => {
                // An unset override does not mean the official CLI has no models.
                parse_help_models(&self.catalog_output(&["--help"]).await?)?
            }
            Err(error) => return Err(error),
        };
        let chat = self.capabilities().await?;
        Ok(models
            .into_iter()
            .map(|model| ModelDescriptor {
                id: format!("codebuddy/{model}"),
                provider_id: "codebuddy".into(),
                connection_id: None,
                cost_mode: CostMode::Unknown,
                supports_streaming: true,
                supports_session: true,
                chat: chat.clone(),
            })
            .collect())
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
fn newer_node_dir() -> Option<PathBuf> {
    let root = PathBuf::from(env::var_os("HOME")?).join(".nvm/versions/node");
    let mut versions = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let parts: Vec<_> = name
                .strip_prefix('v')?
                .split('.')
                .map(str::parse::<u32>)
                .collect();
            if parts.len() != 3 || parts.iter().any(Result::is_err) {
                return None;
            }
            let version = (
                parts[0].as_ref().ok()?,
                parts[1].as_ref().ok()?,
                parts[2].as_ref().ok()?,
            );
            let version = (*version.0, *version.1, *version.2);
            let node = entry.path().join("bin/node");
            (version >= (18, 20, 8) && node.is_file())
                .then_some((version, node.parent()?.to_owned()))
        })
        .collect::<Vec<_>>();
    versions.sort_by_key(|(version, _)| *version);
    versions.pop().map(|(_, path)| path)
}
fn valid_model(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
}
fn parse_selected_model(output: &[u8]) -> Result<String, AdapterError> {
    let raw = std::str::from_utf8(output)
        .map_err(|_| AdapterError::Unsupported)?
        .trim();
    if raw.is_empty() {
        return Err(AdapterError::ModelUnavailable);
    }
    let model = if let Ok(value) = serde_json::from_str::<Value>(raw) {
        value
            .as_str()
            .or_else(|| value["model"].as_str())
            .ok_or(AdapterError::ModelUnavailable)?
            .to_owned()
    } else {
        raw.to_owned()
    };
    if valid_model(&model) {
        Ok(model)
    } else {
        Err(AdapterError::Unsupported)
    }
}
fn parse_help_models(output: &[u8]) -> Result<Vec<String>, AdapterError> {
    let help = std::str::from_utf8(output).map_err(|_| AdapterError::Unsupported)?;
    let section = help
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("--model <model>"))
        .enumerate()
        .take_while(|(index, line)| *index == 0 || !line.trim_start().starts_with('-'))
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join(" ");
    let (_, models) = section
        .split_once("Currently supported: (")
        .ok_or(AdapterError::Unsupported)?;
    let (models, _) = models.split_once(')').ok_or(AdapterError::Unsupported)?;
    let mut ids = Vec::new();
    for model in models.split(',').map(str::trim) {
        if !valid_model(model) {
            return Err(AdapterError::Unsupported);
        }
        if !ids.iter().any(|id| id == model) {
            ids.push(model.to_owned());
        }
    }
    Ok(ids)
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
    } else if message.contains("credit") || message.contains("quota") {
        AdapterError::QuotaExhausted
    } else {
        AdapterError::ProcessFailed
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_model_must_be_exact_identifier() {
        assert_eq!(parse_selected_model(b"gpt-5\n").unwrap(), "gpt-5");
        assert_eq!(
            parse_selected_model(br#"{"model":"custom/x"}"#).unwrap(),
            "custom/x"
        );
        assert!(parse_selected_model(b"Available model: gpt-5").is_err());
        assert!(matches!(
            parse_selected_model(b"\n"),
            Err(AdapterError::ModelUnavailable)
        ));
    }
    #[test]
    fn unset_override_uses_only_explicit_help_model_ids() {
        assert_eq!(parse_help_models(b"  --model <model> Model for the current session. Currently supported: (hy3, glm-5.3,\n kimi-k2.7, hy3)\n  --text-to-image-model <model> images").unwrap(), ["hy3", "glm-5.3", "kimi-k2.7"]);
        assert!(parse_help_models(b"--model <model> model ID").is_err());
        assert!(parse_help_models(b"--model <model> Currently supported: (auto, bad id)").is_err());
        assert!(parse_help_models(b"--model <model> Currently supported: ()").is_err());
    }
    #[test]
    fn only_answer_deltas_are_exposed() {
        assert_eq!(
            parse_delta(
                &serde_json::json!({"type":"stream_event","event":{"delta":{"type":"text_delta","text":"answer"}}})
            ),
            Some("answer".into())
        );
        assert_eq!(
            parse_delta(
                &serde_json::json!({"type":"stream_event","event":{"delta":{"type":"thinking_delta","thinking":"private"}}})
            ),
            None
        );
    }
}
