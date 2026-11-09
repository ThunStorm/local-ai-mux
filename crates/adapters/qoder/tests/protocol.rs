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
                cli_idle_timeout: localmux_adapter_sdk::cli::CLI_IDLE_TIMEOUT,
                model: "qoder/current-model".into(),
                prompt: "hello".into(),
                messages: Vec::new(),
                parameters: Default::default(),
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

#[cfg(unix)]
#[tokio::test]
async fn leading_system_instructions_map_and_developer_is_rejected() {
    use localmux_adapter_sdk::{Adapter, AdapterError, NormalizedMessage, NormalizedRequest};
    let adapter = localmux_adapter_qoder::QoderAdapter::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qoder.sh"),
    );
    let mut request = NormalizedRequest {
        cli_idle_timeout: localmux_adapter_sdk::cli::CLI_IDLE_TIMEOUT,
        model: "qoder/current-model".into(),
        prompt: "question".into(),
        messages: [("system", "one"), ("system", "two"), ("user", "question")]
            .into_iter()
            .map(|(role, content)| NormalizedMessage {
                role: role.into(),
                content: content.into(),
            })
            .collect(),
        parameters: Default::default(),
    };
    let response = adapter.invoke(request.clone()).await.unwrap();
    assert!(response.metadata.usage.is_none());
    assert_eq!(
        response.metadata.finish_reason,
        localmux_adapter_sdk::FinishReason::Unknown
    );
    request.messages[0].role = "developer".into();
    assert!(matches!(
        adapter.invoke(request).await,
        Err(AdapterError::UnsupportedMessages)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn cn_bridge_uses_its_own_model_identity_with_leading_system() {
    use localmux_adapter_sdk::{Adapter, NormalizedMessage, NormalizedRequest};
    let adapter = localmux_adapter_qoder::QoderAdapter::new_cn(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qoder.sh"),
    );
    let models = adapter.list_models().await.unwrap();
    assert!(models.iter().all(|m| m.id.starts_with("qoder-cn/")));
    let response = adapter
        .invoke(NormalizedRequest {
            cli_idle_timeout: localmux_adapter_sdk::cli::CLI_IDLE_TIMEOUT,
            model: "qoder-cn/current-model".into(),
            prompt: "question".into(),
            messages: [("system", "one"), ("system", "two"), ("user", "question")]
                .into_iter()
                .map(|(role, content)| NormalizedMessage {
                    role: role.into(),
                    content: content.into(),
                })
                .collect(),
            parameters: Default::default(),
        })
        .await
        .unwrap();
    assert_eq!(response.text, "first second");
    assert!(response.metadata.actual_model.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn headless_cli_error_result_preserves_first_payload_timeout() {
    use localmux_adapter_sdk::{Adapter, AdapterError, NormalizedRequest, TimeoutLayer};
    let adapter = localmux_adapter_qoder::QoderAdapter::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qoder.sh"),
    );
    let result = adapter
        .invoke(NormalizedRequest {
            cli_idle_timeout: std::time::Duration::from_secs(600),
            model: "qoder/current-model".into(),
            prompt: "first-payload-timeout".into(),
            messages: Vec::new(),
            parameters: Default::default(),
        })
        .await;
    assert!(matches!(result, Err(AdapterError::TimeoutAt {
        layer: TimeoutLayer::UpstreamFirstPayload, limit
    }) if limit == std::time::Duration::from_secs(120)));
}

#[cfg(unix)]
#[tokio::test]
async fn headless_cli_recovers_explicit_stderr_timeout_and_ignores_null_error() {
    use localmux_adapter_sdk::{Adapter, AdapterError, NormalizedRequest, TimeoutLayer};
    let adapter = localmux_adapter_qoder::QoderAdapter::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qoder.sh"),
    );
    let request = |prompt: &str| NormalizedRequest {
        cli_idle_timeout: std::time::Duration::from_secs(600),
        model: "qoder/current-model".into(),
        prompt: prompt.into(),
        messages: Vec::new(),
        parameters: Default::default(),
    };
    assert!(
        matches!(adapter.invoke(request("first-payload-stderr")).await,
        Err(AdapterError::TimeoutAt { layer: TimeoutLayer::UpstreamFirstPayload, limit })
        if limit == std::time::Duration::from_secs(120))
    );
    assert_eq!(
        adapter.invoke(request("null-error")).await.unwrap().text,
        "ok"
    );
}
