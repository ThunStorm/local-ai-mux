use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Response, StatusCode},
    routing::{get, post},
    Json, Router,
};
use futures_util::StreamExt;
use localmux_adapter_sdk::{
    provider_http::HttpProvider, Adapter, NormalizedMessage, NormalizedRequest,
};
use localmux_domain::CostMode;
use secrecy::SecretString;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

async fn models(headers: HeaderMap) -> Json<Value> {
    assert_eq!(headers.get("authorization").unwrap(), "Bearer synthetic");
    Json(
        json!({"data":[{"id":"actual-model","capabilities":{"temperature":true,"top_p":true},"limit":{"output":4096}}]}),
    )
}
async fn chat(
    State(observed): State<Arc<Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response<Body> {
    assert_eq!(headers.get("authorization").unwrap(), "Bearer synthetic");
    observed.lock().await.push(body.clone());
    if body["stream"] == true {
        Response::builder().status(StatusCode::OK).header("content-type", "text/event-stream")
            .body(Body::from("data: {\"choices\":[{\"delta\":{\"content\":\"first \"}}]}\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"second\"}}]}\n\ndata: {\"model\":\"reported-upstream\",\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":4,\"total_tokens\":14}}\n\ndata: [DONE]\n\n")).unwrap()
    } else {
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"model":"reported-upstream","choices":[{"message":{"content":"answer"},"finish_reason":"length"}],"usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14}}).to_string(),
            ))
            .unwrap()
    }
}
#[tokio::test]
async fn provider_uses_exact_configured_endpoint_and_parses_sse() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let app = Router::new()
        .route("/models", get(models))
        .route("/chat/completions", post(chat))
        .with_state(observed.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let provider =
        HttpProvider::new("glm", base, SecretString::from("synthetic".to_owned())).unwrap();
    let models = provider.list_models().await.unwrap();
    assert_eq!(models[0].id, "glm/actual-model");
    assert_eq!(models[0].cost_mode, CostMode::Unknown);
    let request = NormalizedRequest {
        model: "glm/actual-model".into(),
        prompt: "hello".into(),
        messages: [
            ("system", "keep context"),
            ("user", "first"),
            ("assistant", "earlier answer"),
            ("user", "hello"),
        ]
        .into_iter()
        .map(|(role, content)| NormalizedMessage {
            role: role.into(),
            content: content.into(),
        })
        .collect(),
        parameters: localmux_adapter_sdk::ChatParameters {
            temperature: Some(0.7),
            top_p: Some(0.8),
            max_tokens: Some(1024),
            ..Default::default()
        },
    };
    let result = provider.invoke(request.clone()).await.unwrap();
    assert_eq!(result.text, "answer");
    assert_eq!(
        result.metadata.usage.as_ref().unwrap().total_tokens,
        Some(14)
    );
    assert_eq!(
        result.metadata.finish_reason,
        localmux_adapter_sdk::FinishReason::Length
    );
    let mut stream = provider.invoke_stream(request.clone()).await.unwrap();
    let mut text = String::new();
    while let Some(delta) = stream.next().await {
        text.push_str(&delta.unwrap());
    }
    assert_eq!(text, "first second");
    let expected = json!([
        {"role":"system","content":"keep context"},
        {"role":"user","content":"first"},
        {"role":"assistant","content":"earlier answer"},
        {"role":"user","content":"hello"}
    ]);
    let bodies = observed.lock().await;
    assert_eq!(bodies.len(), 2);
    for body in bodies.iter() {
        assert_eq!(body["messages"], expected);
        assert_eq!(body["temperature"], json!(0.7));
        assert_eq!(body["top_p"], json!(0.8));
        assert_eq!(body["max_tokens"], 1024);
    }
    drop(bodies);
    let mut rejected = NormalizedRequest {
        model: "glm/actual-model".into(),
        prompt: "x".into(),
        messages: vec![],
        parameters: localmux_adapter_sdk::ChatParameters {
            max_tokens: Some(4097),
            ..Default::default()
        },
    };
    assert!(matches!(
        provider.invoke(rejected.clone()).await,
        Err(localmux_adapter_sdk::AdapterError::InvalidParameter { .. })
    ));
    rejected.parameters = Default::default();
    provider.invoke(rejected).await.unwrap();
    let bodies = observed.lock().await;
    assert_eq!(bodies.len(), 3);
    assert!(bodies[2].get("temperature").is_none());
    assert!(bodies[2].get("top_p").is_none());
    assert!(bodies[2].get("max_tokens").is_none());
    assert!(HttpProvider::new(
        "ark",
        "http://example.com",
        SecretString::from("synthetic".to_owned())
    )
    .is_err());
    drop(bodies);
    let events = provider.invoke_session_events(request, None).await.unwrap();
    let result = localmux_adapter_sdk::collect_events(events.stream)
        .await
        .unwrap();
    assert_eq!(
        result.metadata.usage.as_ref().unwrap().total_tokens,
        Some(14)
    );
    assert_eq!(
        result.metadata.finish_reason,
        localmux_adapter_sdk::FinishReason::Length
    );
    assert_eq!(
        result.metadata.actual_model.as_deref(),
        Some("reported-upstream")
    );
    server.abort();
}
