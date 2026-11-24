use async_trait::async_trait;
use axum::{body::Body, http::Request};
use futures_util::StreamExt;
use http_body_util::BodyExt;
use localmux_adapter_sdk::{
    Adapter, AdapterError, EventStream, FinishReason, NormalizedRequest, NormalizedResponse,
    ResponseMetadata, StreamEvent, TokenUsage,
};
use localmux_domain::{AdapterKind, ChatCapabilities, CostMode, HealthStatus, ModelDescriptor};
use localmux_gateway::{router, GatewayState};
use localmux_storage::{CallFilter, CallRecord};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

struct Fixture;
fn metadata(prompt: &str) -> ResponseMetadata {
    ResponseMetadata {
        finish_reason: match prompt {
            "length" => FinishReason::Length,
            "filter" => FinishReason::ContentFilter,
            "cancel" => FinishReason::Cancelled,
            _ => FinishReason::Unknown,
        },
        usage: (prompt == "usage").then(|| TokenUsage {
            input_tokens: Some(2),
            output_tokens: Some(3),
            total_tokens: Some(5),
            source: "provider_reported".into(),
            scope: "request".into(),
            ..Default::default()
        }),
        actual_model_source: Some("provider_reported".into()),
        finish_source: Some("provider_reported".into()),
        actual_model: Some("reported/model".into()),
        ..Default::default()
    }
}
#[async_trait]
impl Adapter for Fixture {
    fn id(&self) -> &'static str {
        "fixture"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Provider
    }
    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        Ok(vec![ModelDescriptor {
            id: "fixture/model".into(),
            provider_id: "fixture".into(),
            connection_id: None,
            cost_mode: CostMode::Unknown,
            supports_streaming: true,
            supports_session: false,
            chat: ChatCapabilities::http_text(),
        }])
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        if request.prompt == "oversized" {
            return Ok(NormalizedResponse {
                text: "x".repeat(8 * 1024 * 1024 + 1),
                provider_session_id: None,
                metadata: Default::default(),
            });
        }
        if request.prompt == "failure" {
            return Err(AdapterError::ProviderUnavailable);
        }
        if request.prompt == "cli_timeout" {
            return Err(AdapterError::TimeoutAt {
                layer: localmux_adapter_sdk::TimeoutLayer::CliIdle,
                limit: request.cli_idle_timeout,
            });
        }
        if request.prompt == "timeout" {
            return Err(AdapterError::Timeout);
        }
        if request.prompt == "wait" {
            std::future::pending::<()>().await;
        }
        Ok(NormalizedResponse {
            text: "ANSWER_PRIVATE_SENTINEL".into(),
            provider_session_id: None,
            metadata: metadata(&request.prompt),
        })
    }
    async fn invoke_session_events(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<localmux_adapter_sdk::SessionEventStream, AdapterError> {
        assert!(session.is_none());
        Ok(localmux_adapter_sdk::SessionEventStream {
            stream: self.invoke_events(request).await?,
            provider_session_id: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }
    async fn invoke_events(&self, request: NormalizedRequest) -> Result<EventStream, AdapterError> {
        Ok(Box::pin(async_stream::stream! {
            yield Ok(StreamEvent::Delta("ANSWER_PRIVATE_SENTINEL".into()));
            if request.prompt == "wait" { std::future::pending::<()>().await; }
            if request.prompt == "failure" { yield Err(AdapterError::ProviderUnavailable); return; }
            if request.prompt == "cli_timeout" { yield Err(AdapterError::TimeoutAt { layer: localmux_adapter_sdk::TimeoutLayer::CliIdle, limit: request.cli_idle_timeout }); return; }
            if request.prompt == "timeout" { yield Err(AdapterError::Timeout); return; }
            if request.prompt == "eof" { return; }
            yield Ok(StreamEvent::Completed(metadata(&request.prompt)));
        }))
    }
}
async fn setup() -> (GatewayState, String) {
    let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
    state.register(Arc::new(Fixture)).await;
    let (_, key) = state
        .create_key(&["chat:invoke", "responses:invoke"])
        .await
        .unwrap();
    (state, key)
}
fn request(key: &str, endpoint: &str, prompt: &str, stream: bool, store: bool) -> Request<Body> {
    let body = if endpoint == "chat" {
        json!({"model":"fixture/model", "messages":[{"role":"user","content":prompt}],"stream":stream,"store":store})
    } else {
        json!({"model":"fixture/model","input":prompt,"stream":stream,"store":store})
    };
    let path = if endpoint == "chat" {
        "/v1/chat/completions"
    } else {
        "/v1/responses"
    };
    Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}
async fn rows(state: &GatewayState) -> Vec<CallRecord> {
    localmux_storage::list_calls(&state.pool, &CallFilter::default())
        .await
        .unwrap()
}
#[tokio::test]
async fn both_protocols_streaming_and_nonstreaming_keep_all_outcomes_without_bodies() {
    for endpoint in ["chat", "responses"] {
        for stream in [false, true] {
            for (prompt, status) in [
                ("PROMPT_PRIVATE_SENTINEL", "completed"),
                ("usage", "completed"),
                ("length", "truncated"),
                ("filter", "filtered"),
                ("cancel", "cancelled"),
                ("failure", "failed"),
                ("timeout", "timeout"),
            ] {
                let (state, key) = setup().await;
                let response = router(state.clone())
                    .oneshot(request(&key, endpoint, prompt, stream, true))
                    .await
                    .unwrap();
                response.into_body().collect().await.unwrap();
                let records = rows(&state).await;
                assert_eq!(records.len(), 1);
                let record = &records[0];
                assert_eq!(record.status, status, "{endpoint}/{stream}/{prompt}");
                assert!(record.finished_at.is_some());
                assert!(record.elapsed_ms.is_some());
                assert_eq!(record.resolved_model.as_deref(), Some("fixture/model"));
                if prompt == "usage" {
                    assert_eq!(record.total_tokens, Some(5));
                    assert_eq!(record.actual_model.as_deref(), Some("reported/model"));
                } else {
                    assert_eq!(record.total_tokens, None);
                }
                let stored = serde_json::to_string(&records).unwrap();
                assert!(!stored.contains("PRIVATE_SENTINEL"));
                assert!(!stored.contains(&key));
                if endpoint == "responses" {
                    assert_eq!(
                        localmux_storage::list_conversations(&state.pool)
                            .await
                            .unwrap()
                            .len(),
                        usize::from(status == "completed")
                    );
                }
            }
        }
    }
}
#[tokio::test]
async fn opt_out_covers_success_failure_and_stream_disconnect_and_auth_does_not_create_history() {
    for endpoint in ["chat", "responses"] {
        for stream in [false, true] {
            for prompt in ["usage", "failure", "eof"] {
                let (state, key) = setup().await;
                let response = router(state.clone())
                    .oneshot(request(&key, endpoint, prompt, stream, false))
                    .await
                    .unwrap();
                response.into_body().collect().await.unwrap();
                assert!(rows(&state).await.is_empty());
                assert!(localmux_storage::list_conversations(&state.pool)
                    .await
                    .unwrap()
                    .is_empty());
            }
        }
        let (state, _) = setup().await;
        router(state.clone())
            .oneshot(request("lmx_invalid", endpoint, "usage", false, true))
            .await
            .unwrap()
            .into_body()
            .collect()
            .await
            .unwrap();
        assert!(rows(&state).await.is_empty());
    }
    let (state, key) = setup().await;
    let mut req = request(&key, "chat", "not saved", false, true);
    *req.body_mut() = Body::from(
        json!({"model":key,"messages":[{"role":"user","content":"not saved"}]}).to_string(),
    );
    router(state.clone())
        .oneshot(req)
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap();
    let record = rows(&state).await.remove(0);
    assert_eq!(record.requested_model, "[invalid model]");
    assert!(!serde_json::to_string(&record).unwrap().contains(&key));
}
#[tokio::test]
async fn dropped_polled_and_unpolled_streams_and_handler_futures_finish_as_cancelled() {
    for endpoint in ["chat", "responses"] {
        for poll in [false, true] {
            let (state, key) = setup().await;
            let response = router(state.clone())
                .oneshot(request(&key, endpoint, "wait", true, true))
                .await
                .unwrap();
            assert_eq!(rows(&state).await[0].status, "in_progress");
            let mut stream = response.into_body().into_data_stream();
            if poll {
                stream.next().await.unwrap().unwrap();
            }
            drop(stream);
            wait_terminal(&state, "cancelled").await;
        }
        let (state, key) = setup().await;
        let task = tokio::spawn(
            router(state.clone()).oneshot(request(&key, endpoint, "wait", false, true)),
        );
        for _ in 0..100 {
            if !rows(&state).await.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        task.abort();
        let _ = task.await;
        wait_terminal(&state, "cancelled").await;
    }
}
async fn wait_terminal(state: &GatewayState, status: &str) {
    for _ in 0..100 {
        let records = rows(state).await;
        if records.first().is_some_and(|r| r.status == status) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("Expected {status}: {:?}", rows(state).await);
}
#[tokio::test]
async fn stream_eof_is_interrupted_and_known_parameter_rejection_is_a_failure() {
    for endpoint in ["chat", "responses"] {
        let (state, key) = setup().await;
        router(state.clone())
            .oneshot(request(&key, endpoint, "eof", true, true))
            .await
            .unwrap()
            .into_body()
            .collect()
            .await
            .unwrap();
        assert_eq!(rows(&state).await[0].status, "interrupted");
    }
    let (state, key) = setup().await;
    let mut req = request(&key, "chat", "usage", false, true);
    *req.body_mut() = Body::from(json!({"model":"fixture/model","messages":[{"role":"user","content":"secret"}],"max_tokens":0}).to_string());
    let response = router(state.clone()).oneshot(req).await.unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(rows(&state).await[0].status, "failed");
}
#[tokio::test]
async fn playground_and_short_test_sources_are_distinct_and_short_tests_share_results() {
    let (state, key) = setup().await;
    let mut req = request(&key, "chat", "usage", false, true);
    req.headers_mut()
        .insert("x-localmux-source", "playground".parse().unwrap());
    router(state.clone())
        .oneshot(req)
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap();
    state.verify_reported_model("fixture/model").await.unwrap();
    assert!(state
        .verify_reported_model("fixture/missing")
        .await
        .is_err());
    let records = rows(&state).await;
    assert_eq!(records[0].source, "model_test");
    assert_eq!(records[0].status, "failed");
    assert_eq!(records[1].source, "model_test");
    assert_eq!(records[1].status, "completed");
    assert_eq!(records[2].source, "playground");
}
#[tokio::test]
async fn missing_audit_table_fails_before_generation_but_explicit_opt_out_still_works() {
    let (state, key) = setup().await;
    sqlx::query("DROP TABLE call_records")
        .execute(&state.pool)
        .await
        .unwrap();
    let app = router(state.clone());
    assert_eq!(
        app.clone()
            .oneshot(request(&key, "chat", "usage", false, true))
            .await
            .unwrap()
            .status(),
        500
    );
    assert_eq!(
        app.oneshot(request(&key, "chat", "usage", false, false))
            .await
            .unwrap()
            .status(),
        200
    );
}

#[tokio::test]
async fn terminal_is_durable_before_sdk_stops_and_write_failure_prevents_a_success_terminal() {
    for endpoint in ["chat", "responses"] {
        let (state, key) = setup().await;
        let response = router(state.clone())
            .oneshot(request(&key, endpoint, "usage", true, true))
            .await
            .unwrap();
        let mut stream = response.into_body().into_data_stream();
        while let Some(chunk) = stream.next().await {
            let text = String::from_utf8(chunk.unwrap().to_vec()).unwrap();
            if text.contains("finish_reason") && text.contains("usage_status")
                || text.contains("event: response.completed")
            {
                assert_eq!(rows(&state).await[0].status, "completed");
                break;
            }
        }
        drop(stream);
        assert_eq!(rows(&state).await[0].total_tokens, Some(5));
        let (state, key) = setup().await;
        sqlx::query("CREATE TRIGGER refuse_finish BEFORE UPDATE ON call_records BEGIN SELECT RAISE(FAIL,'fixture'); END;").execute(&state.pool).await.unwrap();
        let response = router(state.clone())
            .oneshot(request(&key, endpoint, "usage", true, true))
            .await
            .unwrap();
        let mut stream = response.into_body().into_data_stream();
        let mut frames = String::new();
        let mut failed = false;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => frames.push_str(std::str::from_utf8(&bytes).unwrap()),
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        assert!(failed);
        assert!(!frames.contains("event: response.completed"));
        assert!(!frames.contains("[DONE]"));
    }
}

#[tokio::test]
async fn enabled_recording_preserves_inputs_answers_partial_outcomes_and_respects_opt_out() {
    for endpoint in ["chat", "responses"] {
        for stream in [false, true] {
            for (prompt, expected) in [
                ("普通问题 user@example.com", "complete"),
                ("length", "partial"),
                ("filter", "partial"),
                ("cancel", "partial"),
                ("failure", "partial"),
                ("timeout", "partial"),
            ] {
                let (state, key) = setup().await;
                localmux_storage::set_recording(&state.pool, true)
                    .await
                    .unwrap();
                router(state.clone())
                    .oneshot(request(&key, endpoint, prompt, stream, true))
                    .await
                    .unwrap()
                    .into_body()
                    .collect()
                    .await
                    .unwrap();
                let row = rows(&state).await.remove(0);
                let content = localmux_storage::call_content(&state.pool, &row.id)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(content.messages[0]["content"], prompt);
                assert_eq!(content.state, expected, "{endpoint}/{stream}/{prompt}");
                assert_eq!(
                    content.answer,
                    if stream || !matches!(prompt, "failure" | "timeout") {
                        "ANSWER_PRIVATE_SENTINEL"
                    } else {
                        ""
                    }
                );
                localmux_storage::delete_calls(&state.pool, Some(&row.id))
                    .await
                    .unwrap();
                assert!(localmux_storage::call_content(&state.pool, &row.id)
                    .await
                    .unwrap()
                    .is_none());
                router(state.clone())
                    .oneshot(request(&key, endpoint, "private", stream, false))
                    .await
                    .unwrap()
                    .into_body()
                    .collect()
                    .await
                    .unwrap();
                assert!(rows(&state).await.is_empty());
                let count: i64 = sqlx::query_scalar("SELECT count(*) FROM call_contents")
                    .fetch_one(&state.pool)
                    .await
                    .unwrap();
                assert_eq!(count, 0);
            }
        }
    }
    let (state, key) = setup().await;
    localmux_storage::set_recording(&state.pool, true)
        .await
        .unwrap();
    let messages = json!([{ "role":"system","content":"保留系统规则"},{"role":"user","content":"上一轮提问"},{"role":"assistant","content":"上一轮回答"},{"role":"user","content":"本轮提问"}]);
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(
            json!({"model":"fixture/model","messages":messages}).to_string(),
        ))
        .unwrap();
    router(state.clone())
        .oneshot(req)
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap();
    assert_eq!(
        localmux_storage::call_content(&state.pool, &rows(&state).await[0].id)
            .await
            .unwrap()
            .unwrap()
            .messages,
        messages
    );
    state.verify_reported_model("fixture/model").await.unwrap();
    assert_eq!(
        localmux_storage::call_content(&state.pool, &rows(&state).await[0].id)
            .await
            .unwrap()
            .unwrap()
            .messages[0]["content"],
        "Reply with OK."
    );
}

struct Controlled {
    receiver: tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<String>>>,
}
#[async_trait]
impl Adapter for Controlled {
    fn id(&self) -> &'static str {
        "fixture"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Provider
    }
    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        Fixture.list_models().await
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        Fixture.invoke(request).await
    }
    async fn invoke_events(
        &self,
        _request: NormalizedRequest,
    ) -> Result<EventStream, AdapterError> {
        let mut receiver = self.receiver.lock().await.take().unwrap();
        Ok(Box::pin(
            async_stream::stream! { while let Some(text)=receiver.recv().await { yield Ok(StreamEvent::Delta(text)); } yield Ok(StreamEvent::Completed(ResponseMetadata::default())); },
        ))
    }
    async fn invoke_session_events(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<localmux_adapter_sdk::SessionEventStream, AdapterError> {
        assert!(session.is_none());
        Ok(localmux_adapter_sdk::SessionEventStream {
            stream: self.invoke_events(request).await?,
            provider_session_id: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }
}
async fn read_until(
    stream: &mut (impl futures_util::Stream<Item = Result<axum::body::Bytes, axum::Error>> + Unpin),
    text: &str,
) {
    for _ in 0..20 {
        let bytes = tokio::time::timeout(std::time::Duration::from_secs(3), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if std::str::from_utf8(&bytes).unwrap().contains(text) {
            return;
        }
    }
    panic!("missing expected fixture frame");
}
#[tokio::test]
async fn turning_off_during_a_stream_stops_capture_and_reenable_never_resumes_that_call() {
    for endpoint in ["chat", "responses"] {
        let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        state
            .register(Arc::new(Controlled {
                receiver: tokio::sync::Mutex::new(Some(rx)),
            }))
            .await;
        let (_, key) = state
            .create_key(&["chat:invoke", "responses:invoke"])
            .await
            .unwrap();
        localmux_storage::set_recording(&state.pool, true)
            .await
            .unwrap();
        let response = router(state.clone())
            .oneshot(request(&key, endpoint, "完整输入", true, true))
            .await
            .unwrap();
        let id = rows(&state).await[0].id.clone();
        let mut stream = response.into_body().into_data_stream();
        tx.send("第一段".into()).unwrap();
        read_until(&mut stream, "第一段").await;
        assert_eq!(
            localmux_storage::call_content(&state.pool, &id)
                .await
                .unwrap()
                .unwrap()
                .answer,
            "第一段"
        );
        localmux_storage::set_recording(&state.pool, false)
            .await
            .unwrap();
        tx.send("关闭期间".into()).unwrap();
        read_until(&mut stream, "关闭期间").await;
        localmux_storage::set_recording(&state.pool, true)
            .await
            .unwrap();
        tx.send("再次开启".into()).unwrap();
        read_until(&mut stream, "再次开启").await;
        drop(tx);
        while let Some(frame) = stream.next().await {
            frame.unwrap();
        }
        let content = localmux_storage::call_content(&state.pool, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(content.answer, "第一段");
        assert_eq!(content.state, "stopped");
        assert_eq!(rows(&state).await[0].status, "completed");
        router(state.clone())
            .oneshot(request(&key, endpoint, "新调用", false, true))
            .await
            .unwrap()
            .into_body()
            .collect()
            .await
            .unwrap();
        assert_eq!(
            localmux_storage::call_content(&state.pool, &rows(&state).await[0].id)
                .await
                .unwrap()
                .unwrap()
                .answer,
            "ANSWER_PRIVATE_SENTINEL"
        );
    }
}
#[tokio::test]
async fn live_capture_redacts_split_credentials_and_keeps_partial_text_on_cancel() {
    let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    state
        .register(Arc::new(Controlled {
            receiver: tokio::sync::Mutex::new(Some(rx)),
        }))
        .await;
    let (_, key) = state.create_key(&["chat:invoke"]).await.unwrap();
    localmux_storage::set_recording(&state.pool, true)
        .await
        .unwrap();
    let prompt = format!("保留邮箱 user@example.com，凭据 {key}，api_key=DO_NOT_KEEP");
    let response = router(state.clone())
        .oneshot(request(&key, "chat", &prompt, true, true))
        .await
        .unwrap();
    let id = rows(&state).await[0].id.clone();
    let mut stream = response.into_body().into_data_stream();
    for piece in ["正文😀 sk-", "proj-abcdef", "ghijklmnopqrst", "uvwxyz"] {
        tx.send(piece.into()).unwrap();
        read_until(
            &mut stream,
            if piece == "正文😀 sk-" {
                "sk-"
            } else {
                piece
            },
        )
        .await;
        let content = localmux_storage::call_content(&state.pool, &id)
            .await
            .unwrap()
            .unwrap();
        assert!(!content.answer.contains("abcdef"));
    }
    drop(stream);
    wait_terminal(&state, "cancelled").await;
    let content = localmux_storage::call_content(&state.pool, &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(content.answer, "正文😀 [REDACTED]");
    assert_eq!(content.state, "partial");
    assert!(content.messages.to_string().contains("user@example.com"));
    assert!(!content.messages.to_string().contains(&key));
    assert!(!content.messages.to_string().contains("DO_NOT_KEEP"));
}
#[tokio::test]
async fn enabling_after_a_call_starts_does_not_retroactively_capture_its_body() {
    let (state, key) = setup().await;
    let task =
        tokio::spawn(router(state.clone()).oneshot(request(&key, "chat", "wait", false, true)));
    for _ in 0..100 {
        if state.active_tasks() > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(state.active_tasks(), 1);
    localmux_storage::set_recording(&state.pool, true)
        .await
        .unwrap();
    let id = rows(&state).await[0].id.clone();
    task.abort();
    let _ = task.await;
    wait_terminal(&state, "cancelled").await;
    assert!(localmux_storage::call_content(&state.pool, &id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn bounded_diagnostics_correlate_requests_without_capturing_prompt_or_inventing_timings() {
    for stream in [false, true] {
        let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
        state.register(Arc::new(Fixture)).await;
        let (_, key) = state.create_key(&["chat:invoke"]).await.unwrap();
        let body = json!({"model":"fixture/model","messages":[{"role":"user","content":"PROMPT_PRIVATE_SENTINEL"}],"stream":stream});
        let length = body.to_string().len() as u64;
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("authorization", format!("Bearer {key}"))
                    .header("X-Request-ID", "sk-forged-private-value")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = response.headers()["X-Request-ID"]
            .to_str()
            .unwrap()
            .to_owned();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let call = localmux_storage::list_calls(&state.pool, &Default::default())
            .await
            .unwrap()
            .remove(0);
        assert_eq!(call.request_id.as_deref(), Some(id.as_str()));
        assert!(id.starts_with("req_"));
        let d = call.diagnostics.unwrap();
        assert_eq!(d.request_bytes, Some(length));
        assert_eq!(d.body_recording_enabled, Some(false));
        assert!(d.adapter_ms.is_some());
        assert!(d.total_elapsed_ms.unwrap() >= d.adapter_ms.unwrap());
        assert_eq!(d.model_execution_ms, None);
        assert_eq!(d.first_output_ms.is_some(), stream);
        assert_eq!(d.upstream_cancel_verified.as_deref(), Some("UNKNOWN"));
        if !stream {
            assert_eq!(d.response_bytes, Some(bytes.len() as u64));
        }
        let encoded = serde_json::to_string(&d).unwrap();
        assert!(encoded.len() <= 8192);
        for secret in [
            "PROMPT_PRIVATE_SENTINEL",
            "ANSWER_PRIVATE_SENTINEL",
            key.as_str(),
            "sk-forged-private-value",
        ] {
            assert!(!encoded.contains(secret));
        }
        assert_eq!(
            state.connection_runtime_status("fixture").await["state"],
            "IDLE"
        );
        assert_eq!(
            state.connection_runtime_status("missing").await["state"],
            "UNAVAILABLE"
        );
    }
}

#[tokio::test]
async fn timeout_layers_and_limits_match_wire_and_records_without_guessing() {
    for endpoint in ["chat", "responses"] {
        for stream in [false, true] {
            for (prompt, layer, limit) in [
                ("cli_timeout", "cli_idle", Some(300000)),
                ("timeout", "unknown", None),
            ] {
                let (state, key) = setup().await;
                state.set_cli_idle_timeout_seconds(300).await.unwrap();
                let response = router(state.clone())
                    .oneshot(request(&key, endpoint, prompt, stream, true))
                    .await
                    .unwrap();
                let bytes = response.into_body().collect().await.unwrap().to_bytes();
                let text = String::from_utf8(bytes.to_vec()).unwrap();
                assert!(text.contains("LMX_PROCESS_TIMEOUT"), "{text}");
                assert!(
                    text.contains(&format!("\"timeout_layer\":\"{layer}\"")),
                    "{text}"
                );
                // Final persistence is scheduled on body completion.
                let mut records = rows(&state).await;
                for _ in 0..100 {
                    if records[0].status != "in_progress" {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    records = rows(&state).await;
                }
                assert_eq!(records[0].status, "timeout");
                let d = records[0].diagnostics.as_ref().unwrap();
                assert_eq!(d.timeout_layer.as_deref(), Some(layer));
                assert_eq!(d.timeout_limit_ms, limit);
                assert_eq!(d.cli_idle_timeout_seconds, Some(300));
                assert_ne!(d.reason_code.as_deref(), Some("overall_deadline"));
            }
        }
    }
}
#[tokio::test]
async fn execution_setting_validates_persists_and_preserves_other_settings() {
    let pool = localmux_storage::open_memory().await.unwrap();
    localmux_storage::save_setting_bool(&pool, "close_to_tray", true)
        .await
        .unwrap();
    let state = GatewayState::new(pool.clone());
    state.load_execution_limits().await.unwrap();
    assert_eq!(state.cli_idle_timeout_seconds(), 180);
    for bad in [0, 29, 1201, u64::MAX] {
        assert!(state.set_cli_idle_timeout_seconds(bad).await.is_err());
    }
    assert_eq!(state.cli_idle_timeout_seconds(), 180);
    for seconds in [30, 600, 300, 1200] {
        state.set_cli_idle_timeout_seconds(seconds).await.unwrap();
    }
    let restarted = GatewayState::new(pool.clone());
    restarted.load_execution_limits().await.unwrap();
    assert_eq!(restarted.cli_idle_timeout_seconds(), 1200);
    assert_eq!(
        localmux_storage::setting_bool(&pool, "close_to_tray")
            .await
            .unwrap(),
        Some(true)
    );
    let response = router(restarted)
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["service"], "LocalAiMux");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["execution_limits"]["cli_idle_timeout_seconds"], 1200);
    assert_eq!(body["execution_limits"]["request_timeout_seconds"], 1200);
}

#[tokio::test]
async fn oversized_nonstream_results_are_rejected_before_body_recording_for_both_endpoints() {
    for endpoint in ["chat", "responses"] {
        let (state, key) = setup().await;
        let response = router(state.clone())
            .oneshot(request(&key, endpoint, "oversized", false, true))
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_GATEWAY);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert!(bytes.len() < 4096);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"]["code"], "LMX_OUTPUT_TOO_LARGE");
        let records = rows(&state).await;
        assert_eq!(
            records[0].error_code.as_deref(),
            Some("LMX_OUTPUT_TOO_LARGE")
        );
        assert_eq!(state.active_tasks(), 0);
    }
}
