use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Response, StatusCode},
    routing::{get, post},
    Json, Router,
};
use futures_util::StreamExt;
use localmux_adapter_sdk::{provider_http::HttpProvider, Adapter, NormalizedRequest};
use localmux_domain::CostMode;
use secrecy::SecretString;
use serde_json::{json, Value};

async fn models(headers: HeaderMap) -> Json<Value> {
    assert_eq!(headers.get("authorization").unwrap(), "Bearer synthetic");
    Json(json!({"data":[{"id":"actual-model"}]}))
}
async fn chat(State(_): State<()>, headers: HeaderMap, Json(body): Json<Value>) -> Response<Body> {
    assert_eq!(headers.get("authorization").unwrap(), "Bearer synthetic");
    if body["stream"] == true {
        Response::builder().status(StatusCode::OK).header("content-type", "text/event-stream")
            .body(Body::from("data: {\"choices\":[{\"delta\":{\"content\":\"first \"}}]}\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"second\"}}]}\n\ndata: [DONE]\n\n")).unwrap()
    } else {
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"choices":[{"message":{"content":"answer"}}]}).to_string(),
            ))
            .unwrap()
    }
}
#[tokio::test]
async fn provider_uses_exact_configured_endpoint_and_parses_sse() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let app = Router::new()
        .route("/models", get(models))
        .route("/chat/completions", post(chat))
        .with_state(());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let provider =
        HttpProvider::new("glm", base, SecretString::from("synthetic".to_owned())).unwrap();
    let models = provider.list_models().await.unwrap();
    assert_eq!(models[0].id, "glm/actual-model");
    assert_eq!(models[0].cost_mode, CostMode::Unknown);
    let request = NormalizedRequest {
        model: "glm/actual-model".into(),
        prompt: "hello".into(),
    };
    assert_eq!(
        provider.invoke(request.clone()).await.unwrap().text,
        "answer"
    );
    let mut stream = provider.invoke_stream(request).await.unwrap();
    let mut text = String::new();
    while let Some(delta) = stream.next().await {
        text.push_str(&delta.unwrap());
    }
    assert_eq!(text, "first second");
    assert!(HttpProvider::new(
        "ark",
        "http://example.com",
        SecretString::from("synthetic".to_owned())
    )
    .is_err());
    server.abort();
}
