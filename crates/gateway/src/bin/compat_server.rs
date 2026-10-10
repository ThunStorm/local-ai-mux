use localmux_gateway::{serve, GatewayState};
use sha2::{Digest, Sha256};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key = std::env::var("LOCALMUX_TEST_API_KEY")?;
    if !key.starts_with("lmx_test_") {
        anyhow::bail!("Only synthetic test keys are accepted");
    }
    let port: u16 = std::env::var("LOCALMUX_TEST_PORT")?.parse()?;
    let pool = localmux_storage::open_memory().await?;
    let hash = format!("{:x}", Sha256::digest(key.as_bytes()));
    localmux_storage::insert_api_key(
        &pool,
        "compat",
        &hash,
        &[
            "models:read".into(),
            "chat:invoke".into(),
            "responses:invoke".into(),
        ],
    )
    .await?;
    let state = GatewayState::mock(pool);
    state
        .register(std::sync::Arc::new(ReliabilityFixture))
        .await;
    serve(state, port).await?;
    Ok(())
}

// Only compiled in the synthetic harness; never registered by the desktop app.
struct ReliabilityFixture;
#[async_trait::async_trait]
impl localmux_adapter_sdk::Adapter for ReliabilityFixture {
    fn id(&self) -> &'static str {
        "fixture"
    }
    fn kind(&self) -> localmux_domain::AdapterKind {
        localmux_domain::AdapterKind::Mock
    }
    async fn health(&self) -> localmux_domain::HealthStatus {
        localmux_domain::HealthStatus::Healthy
    }
    async fn list_models(
        &self,
    ) -> Result<Vec<localmux_domain::ModelDescriptor>, localmux_adapter_sdk::AdapterError> {
        Ok(["length", "cancel", "wait", "error"]
            .into_iter()
            .map(|name| localmux_domain::ModelDescriptor {
                id: format!("fixture/{name}"),
                provider_id: "fixture".into(),
                connection_id: None,
                cost_mode: localmux_domain::CostMode::Unknown,
                supports_streaming: true,
                supports_session: false,
                chat: localmux_domain::ChatCapabilities::http_text(),
            })
            .collect())
    }
    async fn invoke(
        &self,
        request: localmux_adapter_sdk::NormalizedRequest,
    ) -> Result<localmux_adapter_sdk::NormalizedResponse, localmux_adapter_sdk::AdapterError> {
        if request.model == "fixture/error" {
            return Err(localmux_adapter_sdk::AdapterError::RateLimited);
        }
        if request.model == "fixture/wait" {
            std::future::pending::<()>().await;
        }
        Ok(localmux_adapter_sdk::NormalizedResponse {
            text: "fixture-text".into(),
            provider_session_id: None,
            metadata: fixture_metadata(&request.model),
        })
    }
    async fn invoke_session_events(
        &self,
        request: localmux_adapter_sdk::NormalizedRequest,
        session: Option<&str>,
    ) -> Result<localmux_adapter_sdk::SessionEventStream, localmux_adapter_sdk::AdapterError> {
        if session.is_some() {
            return Err(localmux_adapter_sdk::AdapterError::Unsupported);
        }
        Ok(localmux_adapter_sdk::SessionEventStream {
            stream: self.invoke_events(request).await?,
            provider_session_id: Default::default(),
        })
    }
    async fn invoke_events(
        &self,
        request: localmux_adapter_sdk::NormalizedRequest,
    ) -> Result<localmux_adapter_sdk::EventStream, localmux_adapter_sdk::AdapterError> {
        use localmux_adapter_sdk::StreamEvent;
        if request.model == "fixture/error" {
            return Err(localmux_adapter_sdk::AdapterError::RateLimited);
        }
        Ok(Box::pin(async_stream::stream! {
            yield Ok(StreamEvent::Delta("fixture-text".into()));
            if request.model == "fixture/wait" { std::future::pending::<()>().await; }
            yield Ok(StreamEvent::Completed(fixture_metadata(&request.model)));
        }))
    }
}
fn fixture_metadata(model: &str) -> localmux_adapter_sdk::ResponseMetadata {
    use localmux_adapter_sdk::{FinishReason, ResponseMetadata, TokenUsage};
    ResponseMetadata {
        finish_reason: if model == "fixture/cancel" {
            FinishReason::Cancelled
        } else {
            FinishReason::Length
        },
        finish_source: Some("provider_reported".into()),
        actual_model: Some("synthetic-upstream".into()),
        actual_model_source: Some("provider_reported".into()),
        usage: Some(TokenUsage {
            input_tokens: Some(2),
            output_tokens: Some(3),
            total_tokens: Some(5),
            source: "provider_reported".into(),
            scope: "request".into(),
            ..Default::default()
        }),
        ..Default::default()
    }
}
