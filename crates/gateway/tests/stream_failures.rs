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

struct DiagnosticAdapter(Arc<AtomicUsize>, AdapterKind, Arc<AtomicUsize>);
#[async_trait]
impl Adapter for DiagnosticAdapter {
    fn id(&self) -> &'static str {
        "diagnostic"
    }
    fn kind(&self) -> AdapterKind {
        self.1.clone()
    }
    async fn health(&self) -> HealthStatus {
        self.2.fetch_add(1, Ordering::SeqCst);
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
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        if request.prompt == "extended" {
            assert_eq!(request.cli_idle_timeout, Duration::from_secs(1200));
        }
        let seconds = if request.prompt == "never" {
            localmux_gateway::REQUEST_TIMEOUT.as_secs() + 1
        } else if request.prompt == "extended" {
            901
        } else {
            130
        };
        tokio::time::sleep(Duration::from_secs(seconds)).await;
        Ok(NormalizedResponse {
            text: "delayed answer".into(),
            provider_session_id: None,
            metadata: Default::default(),
        })
    }
    async fn invoke_stream(
        &self,
        request: NormalizedRequest,
    ) -> Result<NormalizedStream, AdapterError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        if request.prompt == "extended" {
            assert_eq!(request.cli_idle_timeout, Duration::from_secs(1200));
        }
        Ok(Box::pin(async_stream::try_stream! {
            if request.prompt == "fail" { Err(AdapterError::ProviderUnavailable)?; }
            if request.prompt == "trickle" {
                loop { tokio::time::sleep(Duration::from_secs(100)).await; yield "partial".to_owned(); }
            }
            let seconds = if request.prompt == "never" { 181 } else if request.prompt == "extended" { 901 } else { 35 };
            tokio::time::sleep(Duration::from_secs(seconds)).await;
            yield "delayed answer".to_owned();
        }))
    }
}
fn post(key: &str, path: &str, mut body: serde_json::Value) -> Request<Body> {
    // Virtual-clock tests isolate deadlines/lanes from SQLite worker scheduling.
    // The call_records suite exercises persistence with the real runtime clock.
    body["store"] = serde_json::json!(false);
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
            .register(Arc::new(DiagnosticAdapter(
                Arc::new(AtomicUsize::new(0)),
                AdapterKind::Provider,
                Arc::new(AtomicUsize::new(0)),
            )))
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
        .register(Arc::new(DiagnosticAdapter(
            calls.clone(),
            AdapterKind::Provider,
            Arc::new(AtomicUsize::new(0)),
        )))
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

#[tokio::test]
async fn chat_and_responses_nonstream_allow_long_results_and_bound_total_time() {
    for path in ["/v1/chat/completions", "/v1/responses"] {
        for (prompt, expected) in [
            ("slow", StatusCode::OK),
            ("never", StatusCode::GATEWAY_TIMEOUT),
        ] {
            let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
            let calls = Arc::new(AtomicUsize::new(0));
            state
                .register(Arc::new(DiagnosticAdapter(
                    calls.clone(),
                    AdapterKind::Provider,
                    Arc::new(AtomicUsize::new(0)),
                )))
                .await;
            let (_, key) = state
                .create_key(&["chat:invoke", "responses:invoke"])
                .await
                .unwrap();
            let body = if path == "/v1/responses" {
                serde_json::json!({"model":"diagnostic/luna","input":prompt,"store":false})
            } else {
                serde_json::json!({"model":"diagnostic/luna","messages":[{"role":"user","content":prompt}]})
            };
            tokio::time::pause();
            let pending = tokio::spawn(router(state.clone()).oneshot(post(&key, path, body)));
            // Keep virtual time stationary while SQLite authorization runs on its real worker.
            while calls.load(Ordering::SeqCst) == 0 && !pending.is_finished() {
                tokio::task::yield_now().await;
            }
            let response = pending.await.unwrap().unwrap();
            assert_eq!(response.status(), expected);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let body = String::from_utf8(bytes.to_vec()).unwrap();
            assert!(body.contains(if expected == StatusCode::OK {
                "delayed answer"
            } else {
                "LMX_PROCESS_TIMEOUT"
            }));
            assert_eq!(state.active_tasks(), 0);
            tokio::time::resume();
        }
    }
}

#[tokio::test]
async fn both_streams_stop_at_total_deadline_even_with_regular_deltas() {
    for path in ["/v1/chat/completions", "/v1/responses"] {
        let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
        state
            .register(Arc::new(DiagnosticAdapter(
                Arc::new(AtomicUsize::new(0)),
                AdapterKind::Provider,
                Arc::new(AtomicUsize::new(0)),
            )))
            .await;
        let (_, key) = state
            .create_key(&["chat:invoke", "responses:invoke"])
            .await
            .unwrap();
        let body = if path == "/v1/responses" {
            serde_json::json!({"model":"diagnostic/luna","input":"trickle","stream":true})
        } else {
            chat("trickle", "diagnostic/luna")
        };
        let response = router(state.clone())
            .oneshot(post(&key, path, body))
            .await
            .unwrap();
        tokio::time::pause();
        let started = tokio::time::Instant::now();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("partial") && body.contains("LMX_PROCESS_TIMEOUT"));
        assert!(!body.contains("response.completed"));
        assert!(started.elapsed() < localmux_gateway::REQUEST_TIMEOUT + Duration::from_secs(20));
        assert_eq!(state.active_tasks(), 0);
        tokio::time::resume();
    }
}

#[tokio::test]
async fn agent_lane_is_shared_between_models_keys_and_protocols_and_releases_on_drop() {
    let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let checks = Arc::new(AtomicUsize::new(0));
    state
        .register(Arc::new(DiagnosticAdapter(
            calls.clone(),
            AdapterKind::Agent,
            checks.clone(),
        )))
        .await;
    let (_, key) = state
        .create_key_with_policy(
            &["chat:invoke", "responses:invoke", "agent:invoke"],
            localmux_security::KeyRestrictions {
                execution_profile: localmux_security::ExecutionProfile::ReadOnlyAgent,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let (_, other_key) = state
        .create_key_with_policy(
            &["responses:invoke", "agent:invoke"],
            localmux_security::KeyRestrictions {
                execution_profile: localmux_security::ExecutionProfile::ReadOnlyAgent,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let app = router(state.clone());
    let first = app
        .clone()
        .oneshot(post(
            &key,
            "/v1/chat/completions",
            chat("slow", "diagnostic/luna"),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::pause();
    let before = checks.load(Ordering::SeqCst);
    let pending = tokio::spawn(app.clone().oneshot(post(
        &other_key,
        "/v1/responses",
        serde_json::json!({"model":"diagnostic/sibling","input":"slow","stream":true}),
    )));
    while checks.load(Ordering::SeqCst) == before && !pending.is_finished() {
        tokio::task::yield_now().await;
    }
    let blocked = pending.await.unwrap().unwrap();
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(blocked.headers()["Retry-After"], "5");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(first);
    for _ in 0..1000 {
        if state.active_tasks() == 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(state.active_tasks(), 0);
    let before = calls.load(Ordering::SeqCst);
    let pending = tokio::spawn(app.clone().oneshot(post(
        &key,
        "/v1/chat/completions",
        chat("slow", "diagnostic/luna"),
    )));
    while calls.load(Ordering::SeqCst) == before && !pending.is_finished() {
        tokio::task::yield_now().await;
    }
    let next = pending.await.unwrap().unwrap();
    assert_eq!(next.status(), StatusCode::OK);
    drop(next);
    for _ in 0..1000 {
        if state.active_tasks() == 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(state.active_tasks(), 0);
    // Measure the five-second spacing with a real clock: authorization uses a SQLite worker.
    tokio::time::resume();
    let started = tokio::time::Instant::now();
    let third = app
        .oneshot(post(
            &key,
            "/v1/chat/completions",
            chat("slow", "diagnostic/luna"),
        ))
        .await
        .unwrap();
    assert_eq!(third.status(), StatusCode::OK);
    assert!(started.elapsed() >= Duration::from_secs(5));
    assert!(started.elapsed() < Duration::from_secs(6));
    drop(third);
    for _ in 0..1000 {
        if state.active_tasks() == 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(state.active_tasks(), 0);
}

#[tokio::test]
async fn agent_stream_uses_admission_snapshot_and_can_wait_beyond_old_idle_limit() {
    for path in ["/v1/chat/completions", "/v1/responses"] {
        let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
        state.set_cli_idle_timeout_seconds(300).await.unwrap();
        state
            .register(Arc::new(DiagnosticAdapter(
                Arc::new(AtomicUsize::new(0)),
                AdapterKind::Agent,
                Arc::new(AtomicUsize::new(0)),
            )))
            .await;
        let (_, key) = state
            .create_key_with_policy(
                &["chat:invoke", "responses:invoke", "agent:invoke"],
                localmux_security::KeyRestrictions {
                    execution_profile: localmux_security::ExecutionProfile::ReadOnlyAgent,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let body = if path.ends_with("responses") {
            serde_json::json!({"model":"diagnostic/luna","input":"never","stream":true})
        } else {
            chat("never", "diagnostic/luna")
        };
        let response = router(state.clone())
            .oneshot(post(&key, path, body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        state.set_cli_idle_timeout_seconds(30).await.unwrap();
        tokio::time::pause();
        let text = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(text.contains("delayed answer"), "{text}");
        assert!(!text.contains("LMX_PROCESS_TIMEOUT"), "{text}");
        tokio::time::resume();
    }
}

#[tokio::test]
async fn configured_twenty_minute_budget_allows_fifteen_minute_agent_results() {
    for path in ["/v1/chat/completions", "/v1/responses"] {
        for stream in [false, true] {
            let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
            state.set_cli_idle_timeout_seconds(1200).await.unwrap();
            let calls = Arc::new(AtomicUsize::new(0));
            state
                .register(Arc::new(DiagnosticAdapter(
                    calls.clone(),
                    AdapterKind::Agent,
                    Arc::new(AtomicUsize::new(0)),
                )))
                .await;
            let (_, key) = state
                .create_key_with_policy(
                    &["chat:invoke", "responses:invoke", "agent:invoke"],
                    localmux_security::KeyRestrictions {
                        execution_profile: localmux_security::ExecutionProfile::ReadOnlyAgent,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            let body = if path.ends_with("responses") {
                serde_json::json!({"model":"diagnostic/luna","input":"extended","stream":stream})
            } else {
                serde_json::json!({"model":"diagnostic/luna","messages":[{"role":"user","content":"extended"}],"stream":stream})
            };
            let app = router(state.clone());
            let response = if stream {
                let response = app.oneshot(post(&key, path, body)).await.unwrap();
                state.set_cli_idle_timeout_seconds(30).await.unwrap();
                tokio::time::pause();
                response
            } else {
                tokio::time::pause();
                let pending = tokio::spawn(app.oneshot(post(&key, path, body)));
                while calls.load(Ordering::SeqCst) == 0 && !pending.is_finished() {
                    tokio::task::yield_now().await;
                }
                pending.await.unwrap().unwrap()
            };
            assert_eq!(response.status(), StatusCode::OK);
            let text = String::from_utf8(
                response
                    .into_body()
                    .collect()
                    .await
                    .unwrap()
                    .to_bytes()
                    .to_vec(),
            )
            .unwrap();
            assert!(text.contains("delayed answer"), "{text}");
            assert!(!text.contains("LMX_PROCESS_TIMEOUT"), "{text}");
            tokio::time::resume();
            assert_eq!(state.active_tasks(), 0);
        }
    }
}
