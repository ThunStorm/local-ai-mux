use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use localmux_gateway::{router, GatewayState};
#[cfg(unix)]
use localmux_security::ExecutionProfile;
use localmux_security::KeyRestrictions;
#[cfg(unix)]
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;

fn chat(key: &str, model: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(
            serde_json::json!({"model":model,"messages":[{"role":"user","content":"hello"}]})
                .to_string(),
        ))
        .unwrap()
}
#[cfg(unix)]
fn response(key: &str, model: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(
            serde_json::json!({"model":model,"input":"hello"}).to_string(),
        ))
        .unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn agent_requires_explicit_profile_and_scope_on_both_apis() {
    let pool = localmux_storage::open_memory().await.unwrap();
    let state = GatewayState::mock(pool);
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../adapters/codex/tests/fixtures/app_server.sh");
    state
        .register(Arc::new(localmux_adapter_codex::CodexAdapter::new(fixture)))
        .await;
    let (_, default_key) = state
        .create_key(&["models:read", "chat:invoke", "responses:invoke"])
        .await
        .unwrap();
    let (_, no_scope) = state
        .create_key_with_policy(
            &["chat:invoke", "responses:invoke"],
            KeyRestrictions {
                execution_profile: ExecutionProfile::ReadOnlyAgent,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let (_, allowed) = state
        .create_key_with_policy(
            &["chat:invoke", "responses:invoke", "agent:invoke"],
            KeyRestrictions {
                execution_profile: ExecutionProfile::ReadOnlyAgent,
                allowed_routes: vec!["codex/fixture-model".into()],
                allowed_connections: vec!["codex".into()],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let app = router(state);
    for key in [&default_key, &no_scope] {
        assert_eq!(
            app.clone()
                .oneshot(chat(key, "codex/fixture-model"))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.clone()
                .oneshot(response(key, "codex/fixture-model"))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        app.clone()
            .oneshot(chat(&allowed, "codex/fixture-model"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.oneshot(response(&allowed, "codex/fixture-model"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn route_connection_limit_rate_limit_and_body_limit_are_enforced() {
    let pool = localmux_storage::open_memory().await.unwrap();
    let state = GatewayState::mock(pool.clone());
    let (_, wrong_route) = state
        .create_key_with_policy(
            &["chat:invoke"],
            KeyRestrictions {
                allowed_routes: vec!["other/model".into()],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let (_, wrong_connection) = state
        .create_key_with_policy(
            &["chat:invoke"],
            KeyRestrictions {
                allowed_connections: vec!["other".into()],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let (id, limited) = state
        .create_key_with_policy(
            &["chat:invoke"],
            KeyRestrictions {
                rate_limit_per_minute: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let (_, body_key) = state.create_key(&["chat:invoke"]).await.unwrap();
    let app = router(state);
    assert_eq!(
        app.clone()
            .oneshot(chat(&wrong_route, "mock/echo"))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.clone()
            .oneshot(chat(&wrong_connection, "mock/echo"))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.clone()
            .oneshot(chat(&limited, "mock/echo"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(chat(&limited, "mock/echo"))
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    localmux_storage::revoke_api_key(&pool, &id).await.unwrap();
    assert_eq!(
        app.clone()
            .oneshot(chat(&limited, "mock/echo"))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let oversized = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("authorization", format!("Bearer {body_key}"))
        .body(Body::from("x".repeat(1024 * 1024 + 1)))
        .unwrap();
    assert_eq!(
        app.oneshot(oversized).await.unwrap().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
