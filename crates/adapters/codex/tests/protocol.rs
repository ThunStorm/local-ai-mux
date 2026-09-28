#[cfg(unix)]
#[tokio::test]
async fn official_protocol_shape_handles_catalog_turn_and_resume() {
    use futures_util::StreamExt;
    use localmux_adapter_codex::CodexAdapter;
    use localmux_adapter_sdk::{Adapter, NormalizedRequest};
    use std::path::PathBuf;
    let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app_server.sh");
    let adapter = CodexAdapter::new(executable);
    let models = adapter.connect().await.unwrap();
    assert_eq!(models[0].id, "codex/fixture-model");
    assert_eq!(
        adapter.get_quota().await.unwrap().unwrap().remaining,
        Some(75.0)
    );
    let request = NormalizedRequest {
        model: "codex/fixture-model".into(),
        prompt: "hello".into(),
        messages: Vec::new(),
    };
    let response = adapter
        .invoke_with_session(request.clone(), None)
        .await
        .unwrap();
    assert_eq!(response.text, "fixture reply");
    assert_eq!(
        response.provider_session_id.as_deref(),
        Some("thread-fixture")
    );
    let resumed = adapter
        .invoke_with_session(request, Some("thread-fixture"))
        .await
        .unwrap();
    assert_eq!(resumed.text, "fixture reply");
    let mut stream = adapter
        .invoke_stream(NormalizedRequest {
            model: "codex/fixture-model".into(),
            prompt: "stream".into(),
            messages: Vec::new(),
        })
        .await
        .unwrap();
    assert_eq!(stream.next().await.unwrap().unwrap(), "fixture reply");
    assert!(stream.next().await.is_none());
}
