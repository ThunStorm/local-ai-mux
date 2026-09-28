use async_trait::async_trait;
use futures_util::StreamExt;
use localmux_adapter_sdk::{
    cli::{json_output, stream_json},
    Adapter, AdapterError, NormalizedRequest, NormalizedResponse, NormalizedStream, SessionStream,
};
use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

pub struct QoderAdapter {
    executable: PathBuf,
}
impl QoderAdapter {
    pub fn new(executable: PathBuf) -> Self {
        Self { executable }
    }
    pub async fn invoke_with_session(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<NormalizedResponse, AdapterError> {
        let (mut stream, session_id) = self.stream_with_session(request, session).await?;
        let mut text = String::new();
        while let Some(delta) = stream.next().await {
            text.push_str(&delta?);
        }
        let provider_session_id = session_id
            .lock()
            .await
            .clone()
            .or_else(|| session.map(str::to_owned));
        Ok(NormalizedResponse {
            text,
            provider_session_id,
        })
    }
    async fn stream_with_session(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<(NormalizedStream, Arc<Mutex<Option<String>>>), AdapterError> {
        request.single_user_prompt()?;
        let model = request
            .model
            .strip_prefix("qoder/")
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
        "qoder"
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
        parse_models(
            &json_output(
                &self.executable,
                &["--list-models", "--output-format", "json"],
            )
            .await?,
        )
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
            .map(|(stream, _)| stream)
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
            .map(|(stream, _)| stream)
    }
    async fn invoke_session_stream(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<SessionStream, AdapterError> {
        let (stream, provider_session_id) = self.stream_with_session(request, session).await?;
        Ok(SessionStream {
            stream,
            provider_session_id,
        })
    }
}
fn valid_model(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}
fn parse_models(value: &Value) -> Result<Vec<ModelDescriptor>, AdapterError> {
    let data = value
        .as_array()
        .or_else(|| value["models"].as_array())
        .or_else(|| value["data"].as_array())
        .ok_or(AdapterError::Unsupported)?;
    let models: Vec<_> = data
        .iter()
        .filter_map(|item| {
            let id = item["id"].as_str().or_else(|| item["model"].as_str())?;
            if !valid_model(id) {
                return None;
            }
            Some(ModelDescriptor {
                id: format!("qoder/{id}"),
                provider_id: "qoder".into(),
                connection_id: None,
                cost_mode: CostMode::Unknown,
                supports_streaming: true,
                supports_session: true,
            })
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
        let data =
            serde_json::json!({"models":[{"id":"auto"},{"id":"efficient"},{"name":"not-an-id"}]});
        let models = parse_models(&data).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].cost_mode, CostMode::Unknown);
        assert!(parse_models(&serde_json::json!({"unexpected":"shape"})).is_err());
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
