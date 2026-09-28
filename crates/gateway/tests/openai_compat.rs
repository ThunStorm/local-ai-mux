use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use localmux_gateway::{router, GatewayState};
use tower::ServiceExt;

#[tokio::test]
async fn desktop_gateway_has_no_synthetic_model_before_connection() {
    let pool = localmux_storage::open_memory().await.unwrap();
    let state = GatewayState::new(pool.clone());
    assert!(state.verify_reported_model("mock/echo").await.is_err());
    let (_, key) = state.create_key(&["models:read"]).await.unwrap();
    let response = router(state)
        .oneshot(
            Request::builder()
                .uri("/v1/models")
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn mock_api_requires_key_and_supports_chat_stream_and_revocation() {
    let path = std::env::temp_dir().join(format!("localmux-gateway-{}.db", uuid::Uuid::new_v4()));
    let pool = localmux_storage::open(&path).await.unwrap();
    let state = GatewayState::mock(pool.clone());
    state.verify_reported_model("mock/echo").await.unwrap();
    let app = router(state.clone());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let (id, key) = state
        .create_key(&["models:read", "chat:invoke"])
        .await
        .unwrap();
    let request = Request::builder()
        .uri("/v1/models")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"][0]["id"], "mock/echo");

    for stream in [false, true] {
        let payload = serde_json::json!({"model":"mock/echo","messages":[{"role":"system","content":"context"},{"role":"user","content":"hello"}],"stream":stream});
        let request = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("authorization", format!("Bearer {key}"))
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
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
        assert!(body.contains("hello"));
        assert!(body.contains("context"));
        if stream {
            assert!(body.contains("data: [DONE]"));
        } else {
            assert!(body.contains("chat.completion"));
        }
    }
    let malformed = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from("not-json"))
        .unwrap();
    let response = app.clone().oneshot(malformed).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let raw = std::fs::read(&path).unwrap();
    assert!(!raw.windows(key.len()).any(|part| part == key.as_bytes()));
    localmux_storage::revoke_api_key(&pool, &id).await.unwrap();
    let request = Request::builder()
        .uri("/v1/models")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    pool.close().await;
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn native_adapter_deltas_become_separate_sse_chunks() {
    use async_trait::async_trait;
    use localmux_adapter_sdk::{
        Adapter, AdapterError, NormalizedRequest, NormalizedResponse, NormalizedStream,
    };
    use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
    use std::sync::Arc;
    struct DeltaAdapter;
    #[async_trait]
    impl Adapter for DeltaAdapter {
        fn id(&self) -> &'static str {
            "delta"
        }
        fn kind(&self) -> AdapterKind {
            AdapterKind::Mock
        }
        async fn health(&self) -> HealthStatus {
            HealthStatus::Healthy
        }
        async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
            Ok(vec![ModelDescriptor {
                id: "delta/model".into(),
                provider_id: "delta".into(),
                connection_id: None,
                cost_mode: CostMode::Unknown,
                supports_streaming: true,
                supports_session: false,
            }])
        }
        async fn invoke(
            &self,
            _request: NormalizedRequest,
        ) -> Result<NormalizedResponse, AdapterError> {
            unreachable!()
        }
        async fn invoke_stream(
            &self,
            _request: NormalizedRequest,
        ) -> Result<NormalizedStream, AdapterError> {
            Ok(Box::pin(futures_util::stream::iter(vec![
                Ok("part-one".into()),
                Ok("part-two".into()),
            ])))
        }
    }
    let path = std::env::temp_dir().join(format!("localmux-delta-{}.db", uuid::Uuid::new_v4()));
    let pool = localmux_storage::open(&path).await.unwrap();
    let state = GatewayState::mock(pool.clone());
    state.register(Arc::new(DeltaAdapter)).await;
    let (_, key) = state.create_key(&["chat:invoke"]).await.unwrap();
    let request = Request::builder().method("POST").uri("/v1/chat/completions")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(serde_json::json!({"model":"delta/model","messages":[{"role":"user","content":"hi"}],"stream":true}).to_string())).unwrap();
    let response = router(state).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
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
    assert!(body.find("part-one").unwrap() < body.find("part-two").unwrap());
    assert!(body.contains("data: [DONE]"));
    pool.close().await;
    std::fs::remove_file(path).ok();
}

#[cfg(unix)]
#[tokio::test]
async fn three_offline_adapters_merge_models_and_route_without_collisions() {
    use std::{collections::HashSet, path::PathBuf, sync::Arc};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../adapters");
    let path = std::env::temp_dir().join(format!("localmux-multi-{}.db", uuid::Uuid::new_v4()));
    let pool = localmux_storage::open(&path).await.unwrap();
    let state = GatewayState::mock(pool.clone());
    state
        .register(Arc::new(localmux_adapter_codex::CodexAdapter::new(
            root.join("codex/tests/fixtures/app_server.sh"),
        )))
        .await;
    state
        .register(Arc::new(localmux_adapter_qoder::QoderAdapter::new(
            root.join("qoder/tests/fixtures/qoder.sh"),
        )))
        .await;
    state
        .register(Arc::new(localmux_adapter_codebuddy::CodeBuddyAdapter::new(
            root.join("codebuddy/tests/fixtures/codebuddy.sh"),
        )))
        .await;
    let (_, key) = state
        .create_key_with_policy(
            &["models:read", "chat:invoke", "agent:invoke"],
            localmux_security::KeyRestrictions {
                execution_profile: localmux_security::ExecutionProfile::ReadOnlyAgent,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let app = router(state);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/models")
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let models: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let ids: Vec<&str> = models["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), ids.iter().copied().collect::<HashSet<_>>().len());
    for (model, expected) in [
        ("codex/fixture-model", "fixture reply"),
        ("qoder/current-model", "first second"),
        ("codebuddy/fixture-model", "buddy reply"),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("authorization", format!("Bearer {key}"))
            .body(Body::from(
                serde_json::json!({"model":model,"messages":[{"role":"user","content":"hello"}]})
                    .to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{model}");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["choices"][0]["message"]["content"], expected);
        for stream in [false, true] {
            let request = Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("authorization", format!("Bearer {key}"))
                .body(Body::from(
                    serde_json::json!({"model":model,"stream":stream,"messages":[
                        {"role":"system","content":"keep context"},
                        {"role":"user","content":"first"},
                        {"role":"assistant","content":"earlier answer"},
                        {"role":"user","content":"hello"}
                    ]})
                    .to_string(),
                ))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{model}");
            let body = response.into_body().collect().await.unwrap().to_bytes();
            assert!(String::from_utf8_lossy(&body).contains("LMX_UNSUPPORTED_MESSAGES"));
        }
    }
    pool.close().await;
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn free_pool_tracks_current_classification_without_paid_fallback() {
    use async_trait::async_trait;
    use localmux_adapter_sdk::{Adapter, AdapterError, NormalizedRequest, NormalizedResponse};
    use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    struct ChangingAdapter(Arc<AtomicBool>);
    #[async_trait]
    impl Adapter for ChangingAdapter {
        fn id(&self) -> &'static str {
            "changing"
        }
        fn kind(&self) -> AdapterKind {
            AdapterKind::Mock
        }
        async fn health(&self) -> HealthStatus {
            HealthStatus::Healthy
        }
        async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
            Ok(vec![ModelDescriptor {
                id: "changing/model".into(),
                provider_id: "changing".into(),
                connection_id: None,
                cost_mode: if self.0.load(Ordering::SeqCst) {
                    CostMode::Free
                } else {
                    CostMode::Unknown
                },
                supports_streaming: true,
                supports_session: false,
            }])
        }
        async fn invoke(
            &self,
            _request: NormalizedRequest,
        ) -> Result<NormalizedResponse, AdapterError> {
            Ok(NormalizedResponse {
                text: "free answer".into(),
                provider_session_id: None,
            })
        }
    }
    let path = std::env::temp_dir().join(format!("localmux-free-{}.db", uuid::Uuid::new_v4()));
    let pool = localmux_storage::open(&path).await.unwrap();
    let state = GatewayState::mock(pool.clone());
    let free = Arc::new(AtomicBool::new(false));
    state
        .register(Arc::new(ChangingAdapter(free.clone())))
        .await;
    let (_, key) = state
        .create_key(&["models:read", "chat:invoke"])
        .await
        .unwrap();
    let app = router(state.clone());
    let request = || {
        Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("authorization", format!("Bearer {key}"))
            .body(Body::from(
                serde_json::json!({"model":"free","messages":[{"role":"user","content":"hello"}]})
                    .to_string(),
            ))
            .unwrap()
    };
    assert_eq!(
        app.clone().oneshot(request()).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    free.store(true, Ordering::SeqCst);
    state.invalidate_catalog("changing").await;
    let response = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&body).contains("free answer"));
    free.store(false, Ordering::SeqCst);
    state.invalidate_catalog("changing").await;
    assert_eq!(
        app.oneshot(request()).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    pool.close().await;
    std::fs::remove_file(path).ok();
}
