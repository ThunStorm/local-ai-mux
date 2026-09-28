use async_trait::async_trait;
use futures_util::StreamExt;
use localmux_adapter_sdk::{
    cli::stream_json, Adapter, AdapterError, NormalizedRequest, NormalizedResponse,
    NormalizedStream, SessionStream,
};
use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{process::Command, sync::Mutex, time::timeout};

pub struct CodeBuddyAdapter {
    executable: PathBuf,
}
impl CodeBuddyAdapter {
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
        if let Some(id) = session {
            args.extend(["--resume".to_owned(), id.to_owned()]);
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
        let output = timeout(
            Duration::from_secs(15),
            Command::new(&self.executable)
                .args(["config", "get", "model"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output(),
        )
        .await
        .map_err(|_| AdapterError::ProcessFailed)?
        .map_err(|_| AdapterError::ProviderUnavailable)?;
        if !output.status.success() {
            return Err(AdapterError::Unsupported);
        }
        let model = parse_selected_model(&output.stdout)?;
        Ok(vec![ModelDescriptor {
            id: format!("codebuddy/{model}"),
            provider_id: "codebuddy".into(),
            connection_id: None,
            cost_mode: CostMode::Unknown,
            supports_streaming: true,
            supports_session: true,
        }])
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
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
}
fn parse_selected_model(output: &[u8]) -> Result<String, AdapterError> {
    let raw = std::str::from_utf8(output)
        .map_err(|_| AdapterError::Unsupported)?
        .trim();
    let model = if let Ok(value) = serde_json::from_str::<Value>(raw) {
        value
            .as_str()
            .or_else(|| value["model"].as_str())
            .ok_or(AdapterError::Unsupported)?
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
