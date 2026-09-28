#[cfg(unix)]
#[tokio::test]
async fn selected_model_stream_and_session_fixture() {
    use localmux_adapter_codebuddy::CodeBuddyAdapter;
    use localmux_adapter_sdk::{Adapter, NormalizedRequest};
    use std::path::PathBuf;
    let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codebuddy.sh");
    let adapter = CodeBuddyAdapter::new(executable);
    let models = adapter.list_models().await.unwrap();
    assert_eq!(models[0].id, "codebuddy/fixture-model");
    let response = adapter
        .invoke_with_session(
            NormalizedRequest {
                model: "codebuddy/fixture-model".into(),
                prompt: "hello".into(),
                messages: Vec::new(),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(response.text, "buddy reply");
    assert_eq!(
        response.provider_session_id.as_deref(),
        Some("buddy-fixture-session")
    );
}
