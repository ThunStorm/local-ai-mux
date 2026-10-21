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
                cli_idle_timeout: localmux_adapter_sdk::cli::CLI_IDLE_TIMEOUT,
                model: "codebuddy/fixture-model".into(),
                prompt: "hello".into(),
                messages: Vec::new(),
                parameters: Default::default(),
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

#[cfg(unix)]
#[tokio::test]
async fn unset_model_override_does_not_block_reported_cli_catalog() {
    use localmux_adapter_codebuddy::CodeBuddyAdapter;
    use localmux_adapter_sdk::Adapter;
    use std::path::PathBuf;
    let executable =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unset_model.sh");
    let models = CodeBuddyAdapter::new(executable)
        .list_models()
        .await
        .unwrap();
    assert_eq!(
        models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        ["codebuddy/fixture-a", "codebuddy/fixture-b"]
    );
    assert!(models
        .iter()
        .all(|model| model.cost_mode == localmux_domain::CostMode::Unknown));
}

#[tokio::test]
#[ignore = "read-only catalog check; requires an explicitly supplied official CLI path"]
async fn installed_cli_catalog() {
    use localmux_adapter_codebuddy::CodeBuddyAdapter;
    use localmux_adapter_sdk::Adapter;
    let executable = std::env::var_os("LOCALMUX_CODEBUDDY_CLI").expect("LOCALMUX_CODEBUDDY_CLI");
    let models = CodeBuddyAdapter::new(executable.into())
        .list_models()
        .await
        .unwrap();
    assert!(!models.is_empty());
    assert!(models
        .iter()
        .all(|model| model.id.starts_with("codebuddy/")));
    println!(
        "official CLI reported {} model IDs; no inference performed",
        models.len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn leading_system_instructions_map_and_developer_is_rejected() {
    use localmux_adapter_sdk::{Adapter, AdapterError, NormalizedMessage, NormalizedRequest};
    let adapter = localmux_adapter_codebuddy::CodeBuddyAdapter::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codebuddy.sh"),
    );
    let mut request = NormalizedRequest {
        cli_idle_timeout: localmux_adapter_sdk::cli::CLI_IDLE_TIMEOUT,
        model: "codebuddy/fixture-model".into(),
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
