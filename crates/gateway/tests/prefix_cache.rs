use async_trait::async_trait;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use localmux_adapter_sdk::{
    Adapter, AdapterError, EventStream, NormalizedRequest, NormalizedResponse, ResponseMetadata,
    StreamEvent, TokenUsage,
};
use localmux_domain::{
    AdapterKind, ChatCapabilities, CostMode, HealthStatus, ModelDescriptor, Support,
};
use localmux_gateway::{router, GatewayState};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

struct CacheFixture;
fn usage(case: &str) -> Option<TokenUsage> {
    if case == "unknown" {
        return None;
    }
    Some(TokenUsage {
        input_tokens: (case != "missing-input").then_some(10),
        output_tokens: Some(4),
        total_tokens: Some(14),
        cached_input_tokens: match case {
            "missing" => None,
            "zero" => Some(0),
            "invalid" => Some(11),
            _ => Some(3),
        },
        source: if case == "untrusted" {
            "unverified"
        } else {
            "provider_reported"
        }
        .into(),
        scope: if case == "cumulative" {
            "session_total"
        } else {
            "last_turn"
        }
        .into(),
        ..Default::default()
    })
}
#[async_trait]
impl Adapter for CacheFixture {
    fn id(&self) -> &'static str {
        "cache"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Mock
    }
    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        let mut chat = ChatCapabilities::http_text();
        chat.describe(Some("fixture".into()), "offline");
        chat.describe_session_resume(Support::Supported);
        Ok(vec![ModelDescriptor {
            id: "cache/text".into(),
            provider_id: "cache".into(),
            connection_id: None,
            cost_mode: CostMode::Unknown,
            supports_streaming: true,
            supports_session: true,
            chat,
        }])
    }
    async fn invoke(&self, req: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        Ok(NormalizedResponse {
            text: "answer".into(),
            provider_session_id: None,
            metadata: ResponseMetadata {
                usage: usage(&req.prompt),
                ..Default::default()
            },
        })
    }
    async fn invoke_events(&self, req: NormalizedRequest) -> Result<EventStream, AdapterError> {
        let first = usage(&req.prompt);
        let final_usage = if req.prompt == "cumulative-final" {
            usage("cumulative")
        } else {
            first.clone()
        };
        Ok(Box::pin(async_stream::stream! {
            yield Ok(StreamEvent::Delta("answer".into()));
            if let Some(u) = first { yield Ok(StreamEvent::Usage(u)); }
            yield Ok(StreamEvent::Completed(ResponseMetadata { usage: final_usage, ..Default::default() }));
        }))
    }
    async fn invoke_session_events(
        &self,
        req: NormalizedRequest,
        _: Option<&str>,
    ) -> Result<localmux_adapter_sdk::SessionEventStream, AdapterError> {
        Ok(localmux_adapter_sdk::SessionEventStream {
            stream: self.invoke_events(req).await?,
            provider_session_id: Default::default(),
        })
    }
}
fn post(key: &str, path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}
#[tokio::test]
async fn cache_counts_match_standard_and_extension_across_protocols_and_streams() {
    let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
    state.register(Arc::new(CacheFixture)).await;
    let (_, key) = state
        .create_key(&["chat:invoke", "responses:invoke"])
        .await
        .unwrap();
    let app = router(state.clone());
    for responses in [false, true] {
        for streaming in [false, true] {
            for case in [
                "cached",
                "missing",
                "zero",
                "invalid",
                "missing-input",
                "unknown",
                "untrusted",
                "cumulative",
                "cumulative-final",
            ] {
                let path = if responses {
                    "/v1/responses"
                } else {
                    "/v1/chat/completions"
                };
                let mut request = if responses {
                    json!({"model":"cache/text","input":case})
                } else {
                    json!({"model":"cache/text","messages":[{"role":"user","content":case}]})
                };
                request["stream"] = json!(streaming);
                request["store"] = json!(false);
                if streaming && !responses {
                    request["stream_options"] = json!({"include_usage":true});
                }
                let response = app
                    .clone()
                    .oneshot(post(&key, path, request))
                    .await
                    .unwrap();
                assert!(
                    response.status().is_success(),
                    "{responses}/{streaming}/{case}"
                );
                let bytes = response.into_body().collect().await.unwrap().to_bytes();
                let value: Value = if streaming {
                    let text = std::str::from_utf8(&bytes).unwrap();
                    let last: Value = text
                        .lines()
                        .filter_map(|l| l.strip_prefix("data: "))
                        .filter(|l| *l != "[DONE]")
                        .map(|l| serde_json::from_str::<Value>(l).unwrap())
                        .next_back()
                        .unwrap();
                    if responses {
                        last["response"].clone()
                    } else {
                        last
                    }
                } else {
                    serde_json::from_slice(&bytes).unwrap()
                };
                let trusted = !matches!(case, "unknown" | "untrusted" | "cumulative");
                assert_eq!(
                    value["usage"].is_object(),
                    trusted,
                    "{responses}/{streaming}/{case}"
                );
                assert_eq!(
                    value["localmux"]["usage_status"],
                    if trusted {
                        "provider_reported"
                    } else {
                        "unknown"
                    }
                );
                let expected = match case {
                    "zero" => json!(0),
                    "cached" | "cumulative-final" => json!(3),
                    _ => Value::Null,
                };
                let details = if responses {
                    "input_tokens_details"
                } else {
                    "prompt_tokens_details"
                };
                assert_eq!(
                    value["usage"][details]["cached_tokens"], expected,
                    "{responses}/{streaming}/{case}"
                );
                assert_eq!(value["localmux"]["usage"]["cached_input_tokens"], expected);
                if trusted {
                    assert_eq!(value["usage"]["total_tokens"], 14);
                }
                if expected.is_null() && trusted {
                    assert!(value["usage"].get(details).is_none());
                }
            }
        }
    }
    assert_eq!(state.active_tasks(), 0);
}
#[tokio::test]
async fn capability_catalog_separates_api_resume_and_cache_control_without_accepting_parameters() {
    let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
    state.register(Arc::new(CacheFixture)).await;
    let (_, key) = state
        .create_key(&["models:read", "chat:invoke", "responses:invoke"])
        .await
        .unwrap();
    let app = router(state);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/chat/models")
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let value: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let details = &value["data"][0]["chat"]["details"];
    let mut legacy = details["session_resume"].clone();
    legacy.as_object_mut().unwrap().remove("api_support");
    let legacy: localmux_domain::CapabilityDetail = serde_json::from_value(legacy).unwrap();
    assert!(legacy.api_support.is_empty());
    assert_eq!(
        details["session_resume"]["api_support"]["/v1/responses"],
        "SUPPORTED"
    );
    assert_eq!(
        details["session_resume"]["api_support"]["/v1/chat/completions"],
        "UNSUPPORTED"
    );
    assert_eq!(
        details["prompt_cache_control"]["effective_support"],
        "UNSUPPORTED"
    );
    assert_eq!(details["prompt_cache_control"]["native_support"], "UNKNOWN");
    assert_eq!(
        details["prompt_cache_control"]["evidence"][0]["live_verified"],
        false
    );
    for responses in [false, true] {
        for field in [
            "prompt_cache_options",
            "prompt_cache_key",
            "previous_response_id",
        ] {
            if responses && field == "previous_response_id" {
                continue;
            }
            let path = if responses {
                "/v1/responses"
            } else {
                "/v1/chat/completions"
            };
            let mut request = if responses {
                json!({"model":"cache/text","input":"x"})
            } else {
                json!({"model":"cache/text","messages":[{"role":"user","content":"x"}]})
            };
            request[field] = json!("unmapped");
            assert_eq!(
                app.clone()
                    .oneshot(post(&key, path, request))
                    .await
                    .unwrap()
                    .status(),
                400
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn codex_last_turn_fixture_reaches_both_standard_usage_shapes() {
    for responses in [false, true] {
        for streaming in [false, true] {
            let state = GatewayState::new(localmux_storage::open_memory().await.unwrap());
            state
                .register(Arc::new(localmux_adapter_codex::CodexAdapter::new(
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../adapters/codex/tests/fixtures/app_server.sh"),
                )))
                .await;
            let (_, key) = state
                .create_key_with_policy(
                    &[
                        "models:read",
                        "chat:invoke",
                        "responses:invoke",
                        "agent:invoke",
                    ],
                    localmux_security::KeyRestrictions {
                        execution_profile: localmux_security::ExecutionProfile::ReadOnlyAgent,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            let app = router(state);
            let catalog = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/chat/models")
                        .header("authorization", format!("Bearer {key}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let catalog: Value =
                serde_json::from_slice(&catalog.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(
                catalog["data"][0]["chat"]["details"]["session_resume"]["api_support"]
                    ["/v1/responses"],
                "SUPPORTED"
            );
            let path = if responses {
                "/v1/responses"
            } else {
                "/v1/chat/completions"
            };
            let mut req = if responses {
                json!({"model":"codex/fixture-model","input":"hello"})
            } else {
                json!({"model":"codex/fixture-model","messages":[{"role":"user","content":"hello"}]})
            };
            req["stream"] = json!(streaming);
            req["store"] = json!(false);
            if !responses && streaming {
                req["stream_options"] = json!({"include_usage":true});
            }
            let response = app.oneshot(post(&key, path, req)).await.unwrap();
            assert_eq!(response.status(), 200);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let result: Value = if streaming {
                let last: Value = std::str::from_utf8(&bytes)
                    .unwrap()
                    .lines()
                    .filter_map(|l| l.strip_prefix("data: "))
                    .filter(|l| *l != "[DONE]")
                    .map(|l| serde_json::from_str::<Value>(l).unwrap())
                    .next_back()
                    .unwrap();
                if responses {
                    last["response"].clone()
                } else {
                    last
                }
            } else {
                serde_json::from_slice(&bytes).unwrap()
            };
            let details = if responses {
                "input_tokens_details"
            } else {
                "prompt_tokens_details"
            };
            assert_eq!(result["usage"][details]["cached_tokens"], 3);
            assert_eq!(result["usage"]["total_tokens"], 15);
            assert_eq!(result["localmux"]["usage"]["cached_input_tokens"], 3);
            assert_eq!(result["localmux"]["usage"]["scope"], "last_turn");
        }
    }
}
