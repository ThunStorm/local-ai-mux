pub mod cli;
pub mod provider_http;
use async_trait::async_trait;
use futures_util::{stream, Stream};
use localmux_domain::{AdapterKind, HealthStatus, ModelDescriptor};
use localmux_quota::QuotaSnapshot;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct NormalizedRequest {
    pub model: String,
    pub prompt: String,
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
            text: request.prompt,
            provider_session_id: None,
        })
    }
}
