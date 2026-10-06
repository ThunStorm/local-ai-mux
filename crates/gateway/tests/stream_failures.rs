use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use localmux_adapter_sdk::{
    Adapter, AdapterError, NormalizedRequest, NormalizedResponse, NormalizedStream,
};
use localmux_domain::{AdapterKind, ChatCapabilities, CostMode, HealthStatus, ModelDescriptor};
use localmux_gateway::{router, GatewayState};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tower::ServiceExt;

struct DiagnosticAdapter(Arc<AtomicUsize>);
#[async_trait]
impl Adapter for DiagnosticAdapter {
    fn id(&self) -> &'static str {
        "diagnostic"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Provider
    }
    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        Ok(["luna", "sibling"]
            .into_iter()
            .map(|name| ModelDescriptor {
                id: format!("diagnostic/{name}"),
                provider_id: "diagnostic".into(),
                connection_id: None,
                cost_mode: CostMode::Unknown,
                supports_streaming: true,
                supports_session: false,
                chat: ChatCapabilities::single_turn(),
            })
            .collect())
    }
    async fn invoke(&self, _: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        unreachable!("stream fixture")
    }
    async fn invoke_stream(
        &self,
        request: NormalizedRequest,
    ) -> Result<NormalizedStream, AdapterError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Box::pin(async_stream::try_stream! {
            if request.prompt == "fail" { Err(AdapterError::ProviderUnavailable)?; }
            let seconds = if request.prompt == "never" { 121 } else { 35 };
            tokio::time::sleep(Duration::from_secs(seconds)).await;
            yield "delayed answer".to_owned();
        }))
    }
}
fn post(key: &str, path: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}
fn chat(prompt: &str, model: &str) -> serde_json::Value {
    serde_json::json!({"model":model,"messages":[{"role":"user","content":prompt}],"stream":true})
}
#[tokio::test]
async fn slow_first_text_is_not_cut_off_at_thirty_seconds_and_timeout_is_honest() {
    for prompt in ["slow", "never"] {
        let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
        state
            .register(Arc::new(DiagnosticAdapter(Arc::new(AtomicUsize::new(0)))))
            .await;
        let (_, key) = state.create_key(&["chat:invoke"]).await.unwrap();
        let response = router(state.clone())
            .oneshot(post(
                &key,
                "/v1/chat/completions",
                chat(prompt, "diagnostic/luna"),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        tokio::time::pause();
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(
            body.lines().any(|line| line.starts_with(':')),
            "SSE heartbeat is required during silence"
        );
        if prompt == "slow" {
            assert!(body.contains("delayed answer"));
            assert!(!body.contains("localmux_error"));
        } else {
            assert!(body.contains("LMX_PROCESS_TIMEOUT"));
            assert!(!body.contains("delayed answer"));
        }
        assert_eq!(state.active_tasks(), 0);
        tokio::time::resume();
    }
}
#[tokio::test]
async fn stream_failure_cools_connection_without_claiming_its_models_disappeared() {
    let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    state
        .register(Arc::new(DiagnosticAdapter(calls.clone())))
        .await;
    let (_, key) = state
        .create_key(&["models:read", "chat:invoke", "responses:invoke"])
        .await
        .unwrap();
    let app = router(state.clone());
    let response = app
        .clone()
        .oneshot(post(
            &key,
            "/v1/chat/completions",
            chat("fail", "diagnostic/luna"),
        ))
        .await
        .unwrap();
    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("LMX_PROVIDER_UNAVAILABLE"));
    for (path, body) in [
        ("/v1/chat/completions", chat("fail", "diagnostic/luna")),
        ("/v1/chat/completions", chat("fail", "diagnostic/sibling")),
        (
            "/v1/responses",
            serde_json::json!({"model":"diagnostic/luna","input":"fail"}),
        ),
    ] {
        let response = app.clone().oneshot(post(&key, path, body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["error"]["code"], "LMX_PROVIDER_COOLDOWN");
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "cooldown must not trigger another upstream call"
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/models")
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(body["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|model| model["id"] == "diagnostic/luna"));
    assert_eq!(state.active_tasks(), 0);
}
