pub mod cli;
pub mod provider_http;
use async_trait::async_trait;
use futures_util::{stream, Stream};
use localmux_domain::{AdapterKind, HealthStatus, ModelDescriptor};
use localmux_quota::{QuotaBucket, QuotaSnapshot};
use serde::Serialize;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct NormalizedRequest {
    pub model: String,
    pub prompt: String,
    pub messages: Vec<NormalizedMessage>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NormalizedMessage {
    pub role: String,
    pub content: String,
}
impl NormalizedRequest {
    pub fn single_user_prompt(&self) -> Result<&str, AdapterError> {
        if self.messages.is_empty()
            || (self.messages.len() == 1
                && self.messages[0].role == "user"
                && self.messages[0].content == self.prompt)
        {
            Ok(&self.prompt)
        } else {
            Err(AdapterError::UnsupportedMessages)
        }
    }
    pub fn chat_messages(&self) -> Vec<NormalizedMessage> {
        if self.messages.is_empty() {
            vec![NormalizedMessage {
                role: "user".into(),
                content: self.prompt.clone(),
            }]
        } else {
            self.messages.clone()
        }
    }
}
#[derive(Clone, Debug)]
pub struct NormalizedResponse {
    pub text: String,
    pub provider_session_id: Option<String>,
}
pub type NormalizedStream = Pin<Box<dyn Stream<Item = Result<String, AdapterError>> + Send>>;
pub struct SessionStream {
    pub stream: NormalizedStream,
    pub provider_session_id: Arc<Mutex<Option<String>>>,
}
#[derive(Clone, Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("authentication required")]
    AuthRequired,
    #[error("provider unavailable")]
    ProviderUnavailable,
    #[error("model unavailable")]
    ModelUnavailable,
    #[error("quota exhausted")]
    QuotaExhausted,
    #[error("rate limited")]
    RateLimited,
    #[error("adapter unsupported")]
    Unsupported,
    #[error("message roles or history unsupported")]
    UnsupportedMessages,
    #[error("provider request failed")]
    ProcessFailed,
}

#[async_trait]
pub trait Adapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn kind(&self) -> AdapterKind;
    async fn health(&self) -> HealthStatus;
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError>;
    async fn get_quota(&self) -> Result<Option<QuotaSnapshot>, AdapterError> {
        Ok(None)
    }
    async fn get_quotas(&self) -> Result<Vec<QuotaBucket>, AdapterError> {
        Ok(self
            .get_quota()
            .await?
            .into_iter()
            .map(|snapshot| QuotaBucket {
                connection_id: snapshot.connection_id,
                bucket_id: "legacy".into(),
                label: "上游额度".into(),
                remaining: snapshot.remaining,
                limit: snapshot.limit,
                unit: snapshot.unit,
                reset_at: snapshot.reset_at,
                source: snapshot.source,
                observed_at: snapshot.observed_at,
            })
            .collect())
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError>;
    async fn invoke_stream(
        &self,
        request: NormalizedRequest,
    ) -> Result<NormalizedStream, AdapterError> {
        let response = self.invoke(request).await?;
        Ok(Box::pin(stream::once(async move { Ok(response.text) })))
    }
    async fn resume(
        &self,
        _request: NormalizedRequest,
        _provider_session_id: &str,
    ) -> Result<NormalizedResponse, AdapterError> {
        Err(AdapterError::Unsupported)
    }
    async fn resume_stream(
        &self,
        _request: NormalizedRequest,
        _provider_session_id: &str,
    ) -> Result<NormalizedStream, AdapterError> {
        Err(AdapterError::Unsupported)
    }
    async fn invoke_session_stream(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<SessionStream, AdapterError> {
        if session.is_some() {
            return Err(AdapterError::Unsupported);
        }
        Ok(SessionStream {
            stream: self.invoke_stream(request).await?,
            provider_session_id: Arc::new(Mutex::new(None)),
        })
    }
}

pub struct MockEcho;
#[async_trait]
impl Adapter for MockEcho {
    fn id(&self) -> &'static str {
        "mock"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Mock
    }
    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        Ok(vec![ModelDescriptor {
            id: "mock/echo".into(),
            provider_id: "mock".into(),
            connection_id: None,
            cost_mode: localmux_domain::CostMode::Unknown,
            supports_streaming: true,
            supports_session: false,
        }])
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        if request.model != "mock/echo" {
            return Err(AdapterError::ModelUnavailable);
        }
        Ok(NormalizedResponse {
            text: if request.messages.is_empty()
                || (request.messages.len() == 1 && request.messages[0].role == "user")
            {
                request.prompt
            } else {
                request
                    .messages
                    .iter()
                    .map(|message| format!("{}: {}", message.role, message.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            provider_session_id: None,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::{AdapterError, NormalizedMessage, NormalizedRequest};

    #[test]
    fn agent_role_boundary_rejects_history() {
        let request = NormalizedRequest {
            model: "codex/model".into(),
            prompt: "next".into(),
            messages: vec![
                NormalizedMessage {
                    role: "system".into(),
                    content: "rules".into(),
                },
                NormalizedMessage {
                    role: "user".into(),
                    content: "next".into(),
                },
            ],
        };
        assert!(matches!(
            request.single_user_prompt(),
            Err(AdapterError::UnsupportedMessages)
        ));
    }
}
