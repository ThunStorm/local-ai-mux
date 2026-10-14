#[cfg(unix)]
#[tokio::test]
async fn schema_refresh_handles_future_versions_missing_fields_and_recovery() {
    use localmux_adapter_codex::CodexAdapter;
    use localmux_adapter_sdk::{AdapterError, NormalizedMessage, NormalizedRequest};
    use localmux_domain::Support::{Supported, Unknown};
    use std::os::unix::fs::PermissionsExt;

    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (version, case, system, developer) in [
        ("0.162.0-alpha.17.2", "complete", Supported, Supported),
        ("0.999.0", "complete", Supported, Supported),
        ("0.999.0", "missing_system", Unknown, Supported),
        ("0.999.0", "missing_developer", Supported, Unknown),
        ("0.999.0", "missing_resume", Unknown, Unknown),
        ("0.999.0", "unavailable", Unknown, Unknown),
        ("0.999.0", "malformed", Unknown, Unknown),
        ("0.999.0", "oversized", Unknown, Unknown),
        ("0.999.0", "no_effort", Supported, Supported),
        ("0.999.0", "restricted_effort", Supported, Supported),
        ("0.999.0", "cyclic", Supported, Supported),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let source = std::fs::read_to_string(fixtures.join("app_server.py"))
            .unwrap()
            .replace("codex-cli 0.160.0", &format!("codex-cli {version}"));
        let script = dir.path().join("app_server.py");
        std::fs::write(
            &script,
            source.replace(
                "SCHEMA_CASE = \"complete\"",
                &format!("SCHEMA_CASE = {case:?}"),
            ),
        )
        .unwrap();
        let executable = dir.path().join("app_server.sh");
        std::fs::copy(fixtures.join("app_server.sh"), &executable).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let adapter = CodexAdapter::new(executable);
        let models = adapter.connect().await.unwrap();
        assert_eq!(models[0].chat.system, system, "{case}");
        assert_eq!(models[0].chat.developer, developer, "{case}");
        let efforts: &[&str] = match case {
            "unavailable" | "no_effort" | "cyclic" => &[],
            "restricted_effort" => &["low"],
            _ => &["low", "high"],
        };
        assert_eq!(models[0].chat.reasoning_efforts, efforts, "{case}");
        if efforts.is_empty() {
            assert!(models[0].chat.default_reasoning_effort.is_none());
        }
        assert_eq!(
            models[0].chat.details["system"].evidence[0]
                .version
                .as_deref(),
            Some(version)
        );
        assert_eq!(
            models[0].chat.details["system"].evidence[0].source_type,
            "installed_public_schema_and_adapter_mapping"
        );
        assert!(!models[0].chat.details["system"].evidence[0].live_verified);
        if system == Unknown {
            assert_eq!(
                models[0].chat.details["system"].unknown_reason.as_deref(),
                Some(if case == "unavailable" {
                    "public_schema_unavailable"
                } else {
                    "public_schema_field_unverified"
                })
            );
        }
        let request = NormalizedRequest {
            model: "codex/fixture-model".into(),
            prompt: "hello".into(),
            messages: Vec::new(),
            parameters: Default::default(),
        };
        if !efforts.contains(&"high") {
            let mut unsupported_effort = request.clone();
            unsupported_effort.parameters.reasoning_effort = Some("high".into());
            assert!(
                matches!(
                    adapter.invoke_with_session(unsupported_effort, None).await,
                    Err(AdapterError::InvalidParameter { param, .. }) if param == "reasoning_effort"
                ),
                "{case}"
            );
        }
        // Default user-only calls omit unverified optional fields, even if export fails.
        assert_eq!(
            adapter
                .invoke_with_session(request.clone(), None)
                .await
                .unwrap()
                .text,
            "fixture reply"
        );
        for (role, support) in [("system", system), ("developer", developer)] {
            let mut layered = request.clone();
            layered.messages = vec![
                NormalizedMessage {
                    role: role.into(),
                    content: "retain this rule".into(),
                },
                NormalizedMessage {
                    role: "user".into(),
                    content: "hello".into(),
                },
            ];
            let response = adapter.invoke_with_session(layered, None).await;
            if support == Supported {
                assert_eq!(response.unwrap().text, "fixture reply");
            } else {
                assert!(
                    matches!(response, Err(AdapterError::UnsupportedMessages)),
                    "{case}/{role}"
                );
            }
        }
        // Re-read a changed schema at the same CLI path/version; failed probes never stick.
        std::fs::write(
            &script,
            source.replace("codex-cli 0.999.0", "codex-cli 1.0.0"),
        )
        .unwrap();
        let recovered = adapter.connect().await.unwrap();
        assert_eq!(recovered[0].chat.system, Supported);
        assert_eq!(recovered[0].chat.developer, Supported);
        assert_eq!(recovered[0].chat.reasoning_efforts, ["low", "high"]);
        assert_eq!(
            recovered[0].chat.details["system"].evidence[0]
                .version
                .as_deref(),
            Some(if version == "0.999.0" {
                "1.0.0"
            } else {
                version
            })
        );
    }
}

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
