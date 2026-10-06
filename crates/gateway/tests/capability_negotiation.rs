use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures_util::stream;
use http_body_util::BodyExt;
use localmux_adapter_sdk::{
    Adapter, AdapterError, EventStream, FinishReason, NormalizedRequest, NormalizedResponse,
    ResponseMetadata, StreamEvent, TokenUsage,
};
use localmux_domain::{
    AdapterKind, ChatCapabilities, CostMode, HealthStatus, ModelDescriptor, ParameterCapability,
    Support,
};
use localmux_gateway::{router, GatewayState};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower::ServiceExt;

struct Negotiated(Arc<AtomicUsize>);
fn metadata() -> ResponseMetadata {
    ResponseMetadata {
        finish_reason: FinishReason::Length,
        usage: Some(TokenUsage {
            input_tokens: Some(10),
            output_tokens: Some(4),
            total_tokens: Some(14),
            reasoning_tokens: Some(2),
            source: "provider_reported".into(),
            scope: "request".into(),
            ..Default::default()
        }),
        actual_model: Some("reported-upstream".into()),
        ..Default::default()
    }
}
#[async_trait]
impl Adapter for Negotiated {
    fn id(&self) -> &'static str {
        "negotiated"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Provider
    }
    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        let mut chat = ChatCapabilities::single_turn();
        chat.system = Support::Supported;
        chat.developer = Support::Supported;
        chat.max_completion_tokens = ParameterCapability::range(1.0, 32.0);
        chat.reasoning_efforts = vec!["low".into(), "high".into()];
        chat.describe(Some("fixture-v1".into()), "offline");
        Ok(vec![ModelDescriptor {
            id: "negotiated/text".into(),
            provider_id: "negotiated".into(),
            connection_id: None,
            cost_mode: CostMode::Unknown,
            supports_streaming: true,
            supports_session: false,
            chat,
        }])
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        request.text_instructions(true)?;
        Ok(NormalizedResponse {
            text: "partial answer".into(),
            provider_session_id: None,
            metadata: metadata(),
        })
    }
    async fn invoke_events(&self, request: NormalizedRequest) -> Result<EventStream, AdapterError> {
        self.invoke(request).await?;
        Ok(Box::pin(stream::iter(vec![
            Ok(StreamEvent::Delta("partial answer".into())),
            Ok(StreamEvent::Usage(metadata().usage.unwrap())),
            Ok(StreamEvent::Usage(metadata().usage.unwrap())),
            Ok(StreamEvent::Completed(metadata())),
        ])))
    }
    async fn invoke_session_events(
        &self,
        request: NormalizedRequest,
        _: Option<&str>,
    ) -> Result<localmux_adapter_sdk::SessionEventStream, AdapterError> {
        Ok(localmux_adapter_sdk::SessionEventStream {
            stream: self.invoke_events(request).await?,
            provider_session_id: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }
}
fn post(key: &str, path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}
async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
#[tokio::test]
async fn minimum_text_contract_rejects_invalid_combinations_before_invocation() {
    let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    state.register(Arc::new(Negotiated(calls.clone()))).await;
    let (_, key) = state
        .create_key(&["models:read", "chat:invoke"])
        .await
        .unwrap();
    let app = router(state);
    let base = json!({"model":"negotiated/text","messages":[{"role":"system","content":"one"},{"role":"system","content":"two"},{"role":"developer","content":"rule"},{"role":"user","content":"question"}]});
    let ok = app
        .clone()
        .oneshot(post(&key, "/v1/chat/completions", base.clone()))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    let ok = value(ok).await;
    assert_eq!(ok["usage"]["total_tokens"], 14);
    assert_eq!(ok["choices"][0]["finish_reason"], "length");
    assert_eq!(ok["localmux"]["normalization"], "leading_instructions_v1");
    assert_eq!(ok["localmux"]["actual_model"], "reported-upstream");
    for (field, data, reason) in [
        ("max_tokens", json!(4), "not_implemented"),
        ("max_completion_tokens", json!(33), "out_of_range"),
        ("reasoning_effort", json!("none"), "out_of_range"),
        (
            "response_format",
            json!({"type":"json_object"}),
            "not_implemented",
        ),
    ] {
        let mut body = base.clone();
        body[field] = data;
        let response = app
            .clone()
            .oneshot(post(&key, "/v1/chat/completions", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error = value(response).await;
        assert_eq!(error["error"]["param"], field);
        assert_eq!(error["error"]["reason_code"], reason);
        assert_eq!(error["error"]["retryable"], false);
        assert!(error["error"]["request_id"].is_string());
    }
    let mut conflict = base.clone();
    conflict["max_tokens"] = json!(4);
    conflict["max_completion_tokens"] = json!(4);
    let err = value(
        app.clone()
            .oneshot(post(&key, "/v1/chat/completions", conflict))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(err["error"]["reason_code"], "conflict");
    for messages in [
        json!([{"role":"user","content":"a"},{"role":"assistant","content":"b"},{"role":"user","content":"c"}]),
        json!([{"role":"user","content":"a"},{"role":"system","content":"b"}]),
        json!([{"role":"developer","content":"a"},{"role":"system","content":"b"},{"role":"user","content":"c"}]),
    ] {
        let response = app
            .clone()
            .oneshot(post(
                &key,
                "/v1/chat/completions",
                json!({"model":"negotiated/text","messages":messages}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn typed_chat_and_responses_streams_preserve_usage_snapshot_and_truncation() {
    let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
    state
        .register(Arc::new(Negotiated(Arc::new(AtomicUsize::new(0)))))
        .await;
    let (_, key) = state
        .create_key(&["chat:invoke", "responses:invoke"])
        .await
        .unwrap();
    let app = router(state.clone());
    let response = app.clone().oneshot(post(&key,"/v1/chat/completions",json!({"model":"negotiated/text","messages":[{"role":"user","content":"question"}],"stream":true,"stream_options":{"include_usage":true}}))).await.unwrap();
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
    let events: Vec<Value> = body
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter(|l| *l != "[DONE]")
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        events
            .iter()
            .find(|e| e["choices"][0]["finish_reason"] == "length")
            .unwrap()["localmux"]["actual_model"],
        "reported-upstream"
    );
    assert_eq!(events.last().unwrap()["usage"]["total_tokens"], 14);
    for stream in [false, true] {
        let response = app
            .clone()
            .oneshot(post(
                &key,
                "/v1/responses",
                json!({"model":"negotiated/text","input":"question","stream":stream}),
            ))
            .await
            .unwrap();
        if stream {
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
            assert!(body.contains("event: response.incomplete"));
            assert!(!body.contains("event: response.completed"));
        } else {
            let body = value(response).await;
            assert_eq!(body["status"], "incomplete");
            assert_eq!(body["usage"]["total_tokens"], 14);
            assert_eq!(body["incomplete_details"]["reason"], "max_output_tokens");
        }
    }
    assert_eq!(state.active_tasks(), 0);
}
#[tokio::test]
async fn unknown_usage_and_finish_are_null_for_legacy_sdk_consumers() {
    let state = GatewayState::mock(localmux_storage::open_memory().await.unwrap());
    let (_, key) = state.create_key(&["chat:invoke"]).await.unwrap();
    let result = value(
        router(state)
            .oneshot(post(
                &key,
                "/v1/chat/completions",
                json!({"model":"mock/echo","messages":[{"role":"user","content":"x"}]}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(result["usage"].is_null());
    assert!(result["choices"][0]["finish_reason"].is_null());
    assert!(result["localmux"]["actual_model"].is_null());
}
