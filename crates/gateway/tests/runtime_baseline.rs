//! Synthetic measurement only, never a subscription/provider benchmark.
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use localmux_gateway::{router, GatewayState};
use tower::ServiceExt;

#[tokio::test]
async fn synthetic_call_diagnostics_measurement_and_storage_growth_are_bounded() {
    let path = std::env::temp_dir().join(format!(
        "localmux-runtime-baseline-{}.db",
        uuid::Uuid::new_v4()
    ));
    let pool = localmux_storage::open(&path).await.unwrap();
    let state = GatewayState::mock(pool.clone());
    let (_, key) = state.create_key(&["chat:invoke"]).await.unwrap();
    let app = router(state.clone());
    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(&pool)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut medians = vec![];
    for store in [false, true] {
        let mut elapsed = vec![];
        for _ in 0..20 {
            let request = serde_json::json!({"model":"mock/echo","messages":[{"role":"user","content":"SYNTHETIC_PRIVATE_TEXT"}],"store":store});
            let start = std::time::Instant::now();
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/chat/completions")
                        .header("authorization", format!("Bearer {key}"))
                        .body(Body::from(request.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let data: serde_json::Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert!(data["localmux"].to_string().len() <= 4096);
            elapsed.push(start.elapsed().as_micros());
        }
        elapsed.sort_unstable();
        medians.push(elapsed[elapsed.len() / 2]);
    }
    let records = localmux_storage::list_calls(&pool, &Default::default())
        .await
        .unwrap();
    assert_eq!(records.len(), 20);
    let mut sizes = vec![];
    for call in records {
        let json = serde_json::to_string(&call.diagnostics.unwrap()).unwrap();
        assert!(!json.contains("SYNTHETIC_PRIVATE_TEXT"));
        assert!(json.len() <= 8192);
        sizes.push(json.len());
    }
    let after: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(&pool)
        .await
        .unwrap();
    eprintln!(
        "{}",
        serde_json::json!({"fixture_calls":40,"persisted_calls":20,"no_store_median_us":medians[0],"metadata_median_us":medians[1],"max_diagnostics_bytes":sizes.iter().max(),"sqlite_allocated_growth_bytes":(after-before)*page_size,"provider_calls":0})
    );
    drop(app);
    pool.close().await;
    std::fs::remove_file(path).unwrap();
}
