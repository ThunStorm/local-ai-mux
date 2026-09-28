#[cfg(unix)]
#[tokio::test]
async fn headless_cli_catalog_stream_and_session_fixture() {
    use localmux_adapter_qoder::QoderAdapter;
    use localmux_adapter_sdk::{Adapter, NormalizedRequest};
    use std::path::PathBuf;
    let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qoder.sh");
    let adapter = QoderAdapter::new(executable);
    let models = adapter.list_models().await.unwrap();
    assert_eq!(models[0].id, "qoder/current-model");
    let response = adapter
        .invoke_with_session(
            NormalizedRequest {
                model: "qoder/current-model".into(),
                prompt: "hello".into(),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(response.text, "first second");
    assert_eq!(
        response.provider_session_id.as_deref(),
        Some("qoder-fixture-session")
    );
}
