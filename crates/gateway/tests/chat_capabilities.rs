use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use localmux_adapter_sdk::{Adapter, AdapterError, NormalizedRequest, NormalizedResponse};
use localmux_domain::{
    AdapterKind, ChatCapabilities, CostMode, HealthStatus, ModelDescriptor, ParameterCapability,
};
use localmux_gateway::{router, GatewayState};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

struct ParameterAdapter(Arc<Mutex<Vec<NormalizedRequest>>>);
#[async_trait]
impl Adapter for ParameterAdapter {
    fn id(&self) -> &'static str {
        "params"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Provider
    }
    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        Ok(vec![ModelDescriptor {
            id: "params/reported".into(),
            provider_id: "params".into(),
            connection_id: None,
            cost_mode: CostMode::Unknown,
            supports_streaming: false,
            supports_session: false,
            chat: ChatCapabilities {
                temperature: ParameterCapability::range(0.0, 1.0),
                max_tokens: ParameterCapability::range(1.0, 128.0),
                ..ChatCapabilities::http_text()
            },
        }])
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        self.0.lock().await.push(request);
        Ok(NormalizedResponse {
            text: "offline answer".into(),
            provider_session_id: None,
            metadata: Default::default(),
        })
    }
}
fn request(key: &str, value: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(value.to_string()))
        .unwrap()
}
#[tokio::test]
async fn reported_parameters_pass_through_and_unknown_or_out_of_range_never_invoke() {
    let pool = localmux_storage::open_memory().await.unwrap();
    let state = GatewayState::new(pool.clone());
    let observed = Arc::new(Mutex::new(Vec::new()));
    state
        .register(Arc::new(ParameterAdapter(observed.clone())))
        .await;
    let (_, key) = state
        .create_key(&["models:read", "chat:invoke"])
        .await
        .unwrap();
    let app = router(state.clone());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/chat/models")
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let directory: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(directory["data"][0]["chat"]["temperature"]["max"], 1.0);
    assert_eq!(directory["data"][0]["chat"]["top_p"]["support"], "UNKNOWN");
    let body = serde_json::json!({"model":"params/reported","messages":[{"role":"system","content":"synthetic rules"},{"role":"user","content":"synthetic question"}],"temperature":0.5,"max_tokens":64});
    assert_eq!(
        app.clone()
            .oneshot(request(&key, body.clone()))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    for (field, value) in [
        ("temperature", serde_json::json!(1.5)),
        ("max_tokens", serde_json::json!(129)),
        ("top_p", serde_json::json!(0.7)),
    ] {
        let mut rejected = body.clone();
        rejected[field] = value;
        let response = app.clone().oneshot(request(&key, rejected)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(error["error"]["code"], "LMX_UNSUPPORTED_PARAMETER");
    }
    let requests = observed.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].parameters.max_tokens, Some(64));
    assert_eq!(requests[0].parameters.temperature, Some(0.5));
    assert_eq!(requests[0].messages[0].role, "system");
    assert_eq!(state.active_tasks(), 0);
    let bytes = serde_json::to_vec(&requests[0].parameters).unwrap();
    assert!(!String::from_utf8(bytes).unwrap().contains("top_p"));
}
#[tokio::test]
async fn chat_directory_obeys_key_scope_connection_filters_and_revocation() {
    let state = GatewayState::mock(localmux_storage::open_memory().await.unwrap());
    let (id, key) = state
        .create_key_with_policy(
            &["models:read"],
            localmux_security::KeyRestrictions {
                allowed_connections: vec!["other".into()],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let app = router(state.clone());
    let req = || {
        Request::builder()
            .uri("/api/chat/models")
            .header("authorization", format!("Bearer {key}"))
            .body(Body::empty())
            .unwrap()
    };
    let response = app.clone().oneshot(req()).await.unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value["data"], serde_json::json!([]));
    localmux_storage::revoke_api_key(&state.pool, &id)
        .await
        .unwrap();
    assert_eq!(
        app.oneshot(req()).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}
