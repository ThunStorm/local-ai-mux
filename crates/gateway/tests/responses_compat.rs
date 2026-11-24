use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use localmux_gateway::{router, GatewayState};
use tower::ServiceExt;

#[cfg(unix)]
#[tokio::test]
async fn responses_chain_uses_native_session_and_rejects_stale_branch() {
    use std::{path::PathBuf, sync::Arc};
    let path = std::env::temp_dir().join(format!("localmux-responses-{}.db", uuid::Uuid::new_v4()));
    let pool = localmux_storage::open(&path).await.unwrap();
    let state = GatewayState::mock(pool.clone());
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../adapters/codex/tests/fixtures/app_server.sh");
    state
        .register(Arc::new(localmux_adapter_codex::CodexAdapter::new(fixture)))
        .await;
    let (_, key) = state
        .create_key_with_policy(
            &["responses:invoke", "agent:invoke"],
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
    let app = router(state);
    let send = |input: &str, previous: Option<&str>, stream: bool| {
        Request::builder().method("POST").uri("/v1/responses")
            .header("authorization", format!("Bearer {key}"))
            .body(Body::from(serde_json::json!({"model":"codex/fixture-model","input":input,"previous_response_id":previous,"stream":stream}).to_string())).unwrap()
    };
    let first = app
        .clone()
        .oneshot(send("first", None, false))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first: serde_json::Value =
        serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(first["output"][0]["content"][0]["text"], "fixture reply");
    let first_id = first["id"].as_str().unwrap();
    let cross_key = Request::builder().method("POST").uri("/v1/responses")
        .header("authorization", format!("Bearer {other_key}"))
        .body(Body::from(serde_json::json!({"model":"codex/fixture-model","input":"not my session","previous_response_id":first_id}).to_string())).unwrap();
    assert_eq!(
        app.clone().oneshot(cross_key).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    for store in [false, true] {
        let request = Request::builder().method("POST").uri("/v1/responses")
            .header("authorization", format!("Bearer {key}"))
            .body(Body::from(serde_json::json!({"model":"sk-fixture-sensitive", "input":"change rejected", "previous_response_id":first_id, "store":store}).to_string())).unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::CONFLICT
        );
        let session = localmux_storage::list_conversations(&pool)
            .await
            .unwrap()
            .remove(0);
        let events = localmux_storage::list_run_events(&pool, &session.id)
            .await
            .unwrap();
        assert!(events
            .iter()
            .all(|event| !event.detail_json.contains("fixture-sensitive")));
        assert_eq!(
            localmux_storage::list_run_events(&pool, &session.id)
                .await
                .unwrap()
                .len(),
            usize::from(store)
        );
    }
    let (left, right) = tokio::join!(
        app.clone().oneshot(send("left", Some(first_id), false)),
        app.clone().oneshot(send("right", Some(first_id), false))
    );
    let (left, right) = (left.unwrap(), right.unwrap());
    let (second, stale) = if left.status() == StatusCode::OK {
        (left, right)
    } else {
        (right, left)
    };
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(stale.status(), StatusCode::NOT_FOUND);
    let second: serde_json::Value =
        serde_json::from_slice(&second.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let second_id = second["id"].as_str().unwrap();
    assert_eq!(
        app.clone()
            .oneshot(send("stale", Some(first_id), false))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let streamed = app
        .clone()
        .oneshot(send("stream", Some(second_id), true))
        .await
        .unwrap();
    assert_eq!(streamed.status(), StatusCode::OK);
    let body = String::from_utf8(
        streamed
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("event: response.output_text.delta"));
    assert!(body.contains("event: response.completed"));
    let sessions = localmux_storage::list_conversations(&pool).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].provider_session_id.as_deref(),
        Some("thread-fixture")
    );
    let calls = localmux_storage::list_calls(
        &pool,
        &localmux_storage::CallFilter {
            conversation_id: Some(sessions[0].id.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(matches!(calls.len(), 4 | 5));
    assert_eq!(calls.iter().filter(|r| r.status == "completed").count(), 3);
    assert_eq!(
        calls.iter().filter(|r| r.status == "failed").count(),
        calls.len() - 3
    );
    sqlx::query("CREATE TRIGGER reject_terminal BEFORE UPDATE OF status ON call_records WHEN NEW.status='completed' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END").execute(&pool).await.unwrap();
    let count = || sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM responses").fetch_one(&pool);
    let before = count().await.unwrap();
    assert_eq!(
        app.clone()
            .oneshot(send("failed", None, false))
            .await
            .unwrap()
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let failed_stream = app
        .oneshot(send("failed stream", None, true))
        .await
        .unwrap();
    assert!(failed_stream.into_body().collect().await.is_err());
    assert_eq!(count().await.unwrap(), before);
    pool.close().await;
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn stateless_mock_response_cannot_fake_resume() {
    let path = std::env::temp_dir().join(format!(
        "localmux-responses-mock-{}.db",
        uuid::Uuid::new_v4()
    ));
    let pool = localmux_storage::open(&path).await.unwrap();
    let state = GatewayState::mock(pool.clone());
    let (_, key) = state.create_key(&["responses:invoke"]).await.unwrap();
    let app = router(state);
    let request = |previous: Option<&str>| {
        Request::builder().method("POST").uri("/v1/responses")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(serde_json::json!({"model":"mock/echo","input":"hello","previous_response_id":previous}).to_string())).unwrap()
    };
    let first = app.clone().oneshot(request(None)).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first: serde_json::Value =
        serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let id = first["id"].as_str().unwrap();
    assert_eq!(
        app.oneshot(request(Some(id))).await.unwrap().status(),
        StatusCode::CONFLICT
    );
    pool.close().await;
    std::fs::remove_file(path).ok();
}
