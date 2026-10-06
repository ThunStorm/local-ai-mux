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
        parameters: Default::default(),
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
            parameters: Default::default(),
        })
        .await
        .unwrap();
    assert_eq!(stream.next().await.unwrap().unwrap(), "fixture reply");
    assert!(stream.next().await.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn upstream_error_notification_fails_promptly_without_waiting_for_retry() {
    use futures_util::StreamExt;
    use localmux_adapter_codex::CodexAdapter;
    use localmux_adapter_sdk::{Adapter, AdapterError, NormalizedRequest};
    let adapter = CodexAdapter::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app_server.sh"),
    );
    let request = NormalizedRequest {
        model: "codex/fixture-model".into(),
        prompt: "error-fixture".into(),
        messages: Vec::new(),
        parameters: Default::default(),
    };
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        adapter.invoke(request.clone()),
    )
    .await
    .unwrap();
    assert!(matches!(response, Err(AdapterError::ProviderUnavailable)));
    let mut stream = adapter.invoke_stream(request).await.unwrap();
    let delta = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
        .await
        .unwrap();
    assert!(matches!(
        delta,
        Some(Err(AdapterError::ProviderUnavailable))
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn layered_instructions_effort_and_last_turn_usage_preserve_public_semantics() {
    use localmux_adapter_codex::CodexAdapter;
    use localmux_adapter_sdk::{
        Adapter, ChatParameters, FinishReason, NormalizedMessage, NormalizedRequest,
    };
    let adapter = CodexAdapter::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app_server.sh"),
    );
    let request = NormalizedRequest {
        model: "codex/fixture-model".into(),
        prompt: "negotiation-fixture".into(),
        messages: [
            ("system", "one"),
            ("system", "two"),
            ("developer", "developer rule"),
            ("user", "negotiation-fixture"),
        ]
        .into_iter()
        .map(|(role, content)| NormalizedMessage {
            role: role.into(),
            content: content.into(),
        })
        .collect(),
        parameters: ChatParameters {
            reasoning_effort: Some("high".into()),
            ..Default::default()
        },
    };
    let response = adapter.invoke(request).await.unwrap();
    assert_eq!(response.metadata.usage.unwrap().total_tokens, Some(15));
    assert_eq!(response.metadata.finish_reason, FinishReason::Unknown);
    assert_eq!(
        response.metadata.actual_model.as_deref(),
        Some("fixture-model")
    );
    let models = adapter.list_models().await.unwrap();
    assert_eq!(models[0].chat.native_input_modalities, ["text", "image"]);
    assert_eq!(models[0].chat.reasoning_efforts, ["low", "high"]);
}
