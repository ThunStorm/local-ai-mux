//! Opt-in, metadata-only check of already connected public runtimes. Never invokes a model.
use localmux_adapter_sdk::Adapter;
use localmux_domain::Support;
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::PathBuf};

#[tokio::test]
#[ignore = "requires explicit CLI paths; public catalog and help only, no inference or connection writes"]
async fn installed_capability_catalog_preserves_all_audited_ids_and_model_metadata() {
    let path = |name| PathBuf::from(std::env::var_os(name).expect(name));
    let adapters: Vec<Box<dyn Adapter>> = vec![
        Box::new(localmux_adapter_codex::CodexAdapter::new(path(
            "LOCALMUX_CATALOG_CODEX",
        ))),
        Box::new(localmux_adapter_qoder::QoderAdapter::new(path(
            "LOCALMUX_CATALOG_QODER",
        ))),
        Box::new(localmux_adapter_qoder::QoderAdapter::new_cn(path(
            "LOCALMUX_CATALOG_QODER_CN",
        ))),
        Box::new(localmux_adapter_codebuddy::CodeBuddyAdapter::new(path(
            "LOCALMUX_CATALOG_CODEBUDDY",
        ))),
    ];
    let audit: Value = serde_json::from_str(include_str!(
        "../../../docs/architecture/capability-audit-20261006.json"
    ))
    .unwrap();
    let expected: BTreeSet<String> = audit["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().into())
        .collect();
    let mut actual = BTreeSet::new();
    let mut evidence = Vec::new();
    for adapter in adapters {
        let models = adapter.list_models().await.unwrap();
        println!(
            "{}: {} public catalog IDs; no inference",
            adapter.id(),
            models.len()
        );
        for model in models {
            assert!(actual.insert(model.id.clone()), "duplicate ID");
            assert_eq!(model.chat.system, Support::Supported);
            assert_eq!(model.chat.history, Support::Unsupported);
            assert!(model
                .chat
                .details
                .values()
                .all(|d| d.evidence.iter().all(|e| !e.live_verified)));
            if adapter.id() == "codex" {
                let native = audit["codex_model_metadata"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|m| model.id == format!("codex/{}", m["model"].as_str().unwrap()))
                    .unwrap();
                assert_eq!(
                    json!(model.chat.native_input_modalities),
                    native["input_modalities"]
                );
                assert_eq!(
                    json!(model.chat.reasoning_efforts),
                    native["reasoning_efforts"]
                );
                assert_eq!(
                    json!(model.chat.default_reasoning_effort),
                    native["default_reasoning_effort"]
                );
            }
            evidence.push(json!({"id":model.id,"system":model.chat.system,"developer":model.chat.developer,"native_input_modalities":model.chat.native_input_modalities,"reasoning_efforts":model.chat.reasoning_efforts,"default_reasoning_effort":model.chat.default_reasoning_effort,"max_tokens":model.chat.max_tokens,"max_completion_tokens":model.chat.max_completion_tokens,"live_verified":false}));
        }
    }
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 55);
    if let Some(path) = std::env::var_os("LOCALMUX_CATALOG_EVIDENCE") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"schema_version":2,"inference_requests_sent":0,"catalog_sets_equal":true,"count":55,"models":evidence})).unwrap()).unwrap();
    }
}
