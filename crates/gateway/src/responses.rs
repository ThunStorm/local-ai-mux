use super::{adapter_error, authorized, error, now, GatewayState};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures_util::StreamExt;
use localmux_adapter_sdk::{Adapter, AdapterError, NormalizedRequest};
use localmux_api_types::{response_input_text, ResponsesRequest};
use localmux_domain::{CostMode, HealthStatus};
use localmux_router::{RouteCandidate, RouteError};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

struct Context {
    adapter: Arc<dyn Adapter>,
    model: String,
    conversation_id: String,
    provider_session_id: Option<String>,
}

async fn context(
    state: &GatewayState,
    owner_key_id: &str,
    requested: &str,
    previous_id: Option<&str>,
) -> Result<Context, Box<Response>> {
    let adapters = state.adapters.read().await.clone();
    if let Some(previous_id) = previous_id {
        let mapping =
            localmux_storage::response_conversation(&state.pool, previous_id, owner_key_id)
                .await
                .map_err(|_| {
                    Box::new(error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "LMX_INTERNAL_ERROR",
                        "Session lookup failed",
                    ))
                })?
                .ok_or_else(|| {
                    Box::new(error(
                        StatusCode::NOT_FOUND,
                        "LMX_SESSION_NOT_FOUND",
                        "Previous response not found or not latest",
                    ))
                })?;
        if !mapping.resume_supported || mapping.provider_session_id.is_none() {
            return Err(Box::new(error(
                StatusCode::CONFLICT,
                "LMX_SESSION_RESUME_FAILED",
                "Upstream model does not support resume",
            )));
        }
        let virtual_route = matches!(requested, "free" | "cheap" | "balanced" | "best");
        if !virtual_route && requested != mapping.model_id {
            let _ = localmux_storage::record_run_event(
                &state.pool,
                &mapping.id,
                "ROUTE_CHANGE_REJECTED",
                &json!({"requested_model":requested}).to_string(),
                now(),
            )
            .await;
            return Err(Box::new(error(
                StatusCode::CONFLICT,
                "LMX_SESSION_RESUME_FAILED",
                "Model change requires a new conversation",
            )));
        }
        let Some(adapter) = adapters
            .into_iter()
            .find(|item| item.id() == mapping.adapter_id)
        else {
            return Err(Box::new(error(
                StatusCode::BAD_GATEWAY,
                "LMX_SESSION_RESUME_FAILED",
                "Sticky provider is unavailable",
            )));
        };
        let candidate = if virtual_route {
            state
                .available_candidates()
                .await
                .into_iter()
                .find(|item| item.model_id == mapping.model_id)
        } else {
            Some(RouteCandidate {
                model_id: mapping.model_id.clone(),
                connection_id: mapping.adapter_id.clone(),
                cost_mode: CostMode::Unknown,
                quota_multiplier: None,
                reserve_available: false,
                health: adapter.health().await,
            })
        };
        let Some(candidate) = candidate else {
            return Err(Box::new(error(
                StatusCode::BAD_GATEWAY,
                "LMX_SESSION_RESUME_FAILED",
                "Sticky model is unavailable",
            )));
        };
        if let Err(reason) = localmux_router::select(
            if virtual_route {
                requested
            } else {
                &mapping.model_id
            },
            &[candidate],
            &state.route_policy().await,
            &state.breaker,
            Instant::now(),
        ) {
            let _ = localmux_storage::record_run_event(
                &state.pool,
                &mapping.id,
                "STICKY_ROUTE_UNAVAILABLE",
                &json!({"reason":reason.to_string()}).to_string(),
                now(),
            )
            .await;
            return Err(Box::new(if reason == RouteError::FreePoolExhausted {
                error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "LMX_FREE_POOL_EXHAUSTED",
                    "Free sticky model is no longer eligible",
                )
            } else {
                error(
                    StatusCode::BAD_GATEWAY,
                    "LMX_SESSION_RESUME_FAILED",
                    "Sticky route unavailable",
                )
            }));
        }
        return Ok(Context {
            adapter,
            model: mapping.model_id,
            conversation_id: mapping.id,
            provider_session_id: mapping.provider_session_id,
        });
    }
    let actual_model = if matches!(requested, "free" | "cheap" | "balanced" | "best") {
        let candidates = state.available_candidates().await;
        localmux_router::select(
            requested,
            &candidates,
            &state.route_policy().await,
            &state.breaker,
            Instant::now(),
        )
        .map_err(|reason| {
            Box::new(match reason {
                RouteError::FreePoolExhausted => error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "LMX_FREE_POOL_EXHAUSTED",
                    "No confirmed free model is available",
                ),
                _ => error(
                    StatusCode::NOT_FOUND,
                    "LMX_MODEL_UNAVAILABLE",
                    "No eligible route",
                ),
            })
        })?
        .model_id
        .clone()
    } else {
        requested.to_owned()
    };
    let adapter = adapters
        .into_iter()
        .find(|item| actual_model.starts_with(&format!("{}/", item.id())))
        .ok_or_else(|| {
            Box::new(error(
                StatusCode::NOT_FOUND,
                "LMX_MODEL_UNAVAILABLE",
                "Model unavailable",
            ))
        })?;
    let candidate = [RouteCandidate {
        model_id: actual_model.clone(),
        connection_id: adapter.id().into(),
        cost_mode: CostMode::Unknown,
        quota_multiplier: None,
        reserve_available: false,
        health: adapter.health().await,
    }];
    if !state.breaker.available(adapter.id(), Instant::now())
        || matches!(
            candidate[0].health,
            HealthStatus::AuthRequired
                | HealthStatus::Offline
                | HealthStatus::Unsupported
                | HealthStatus::QuotaExhausted
        )
    {
        return Err(Box::new(error(
            StatusCode::BAD_GATEWAY,
            "LMX_PROVIDER_UNAVAILABLE",
            "Provider unavailable",
        )));
    }
    Ok(Context {
        adapter,
        model: actual_model,
        conversation_id: Uuid::new_v4().to_string(),
        provider_session_id: None,
    })
}

fn response_object(
    id: &str,
    message_id: &str,
    created: i64,
    model: &str,
    text: &str,
    previous: Option<&str>,
    metadata: &Value,
) -> Value {
    json!({"id":id,"object":"response","created_at":created,"status":"completed","error":null,"incomplete_details":null,
        "model":model,"previous_response_id":previous,"metadata":metadata,
        "output":[{"id":message_id,"type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":text,"annotations":[]}]}],
        "output_text":text,"usage":null})
}
fn event(name: &str, data: Value) -> Event {
    Event::default().event(name).data(data.to_string())
}

pub async fn create(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let key = match authorized(&state, &headers, "responses:invoke").await {
        Ok(key) => key,
        Err(response) => return *response,
    };
    let request: ResponsesRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "LMX_INVALID_REQUEST",
                "Malformed response request",
            )
        }
    };
    let Some(prompt) = response_input_text(&request.input) else {
        return error(
            StatusCode::BAD_REQUEST,
            "LMX_INVALID_REQUEST",
            "Only text input is supported",
        );
    };
    if request.model.is_empty() || prompt.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "LMX_INVALID_REQUEST",
            "Model and text input are required",
        );
    }
    if request.metadata.as_ref().is_some_and(|metadata| {
        metadata.len() > 16
            || metadata
                .iter()
                .any(|(key, value)| key.len() > 64 || value.len() > 512)
    }) {
        return error(
            StatusCode::BAD_REQUEST,
            "LMX_INVALID_REQUEST",
            "Metadata exceeds limits",
        );
    }
    let context = match context(
        &state,
        &key.id,
        &request.model,
        request.previous_response_id.as_deref(),
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if !super::target_allowed(
        &key,
        &request.model,
        &context.model,
        context.adapter.as_ref(),
    ) {
        return error(
            StatusCode::FORBIDDEN,
            "LMX_PERMISSION_DENIED",
            "API key cannot invoke this route",
        );
    }
    let metadata =
        serde_json::to_value(request.metadata.unwrap_or_default()).unwrap_or_else(|_| json!({}));
    let response_id = format!("resp_{}", Uuid::new_v4().simple());
    let message_id = format!("msg_{}", Uuid::new_v4().simple());
    let created = now();
    let normalized = NormalizedRequest {
        model: context.model.clone(),
        prompt,
    };
    let run = state.begin_run();
    if request.stream {
        let session = match tokio::time::timeout(
            Duration::from_secs(30),
            context
                .adapter
                .invoke_session_stream(normalized, context.provider_session_id.as_deref()),
        )
        .await
        {
            Ok(Ok(session)) => session,
            Ok(Err(AdapterError::Unsupported)) if context.provider_session_id.is_some() => {
                return error(
                    StatusCode::CONFLICT,
                    "LMX_SESSION_RESUME_FAILED",
                    "Upstream resume unsupported",
                )
            }
            Ok(Err(kind)) => return adapter_error(kind),
            Err(_) => {
                return error(
                    StatusCode::GATEWAY_TIMEOUT,
                    "LMX_PROCESS_TIMEOUT",
                    "Upstream start timed out",
                )
            }
        };
        let pool = state.pool.clone();
        let owner_key_id = key.id.clone();
        let previous = request.previous_response_id;
        let store = request.store.unwrap_or(true);
        let events = async_stream::stream! {
            let _run = run;
            let mut provider_stream = session.stream;
            let mut text = String::new();
            let mut sequence = 0_u64;
            yield Ok::<Event, Infallible>(event("response.created", json!({"type":"response.created","sequence_number":sequence,"response":{"id":response_id,"object":"response","created_at":created,"status":"in_progress","model":context.model,"output":[]}})));
            sequence += 1;
            yield Ok(event("response.output_item.added", json!({"type":"response.output_item.added","sequence_number":sequence,"output_index":0,"item":{"id":message_id,"type":"message","status":"in_progress","role":"assistant","content":[]}})));
            sequence += 1;
            yield Ok(event("response.content_part.added", json!({"type":"response.content_part.added","sequence_number":sequence,"item_id":message_id,"output_index":0,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}})));
            sequence += 1;
            let mut failed = false;
            while let Some(next) = provider_stream.next().await {
                match next {
                    Ok(delta) => {
                        if text.len() + delta.len() > 8 * 1024 * 1024 { failed = true; break; }
                        text.push_str(&delta);
                        yield Ok(event("response.output_text.delta", json!({"type":"response.output_text.delta","sequence_number":sequence,"item_id":message_id,"output_index":0,"content_index":0,"delta":delta})));
                        sequence += 1;
                    },
                    Err(_) => { failed = true; break; },
                }
            }
            if failed {
                yield Ok(event("response.failed", json!({"type":"response.failed","sequence_number":sequence,"response":{"id":response_id,"status":"failed","error":{"code":"LMX_PROVIDER_UNAVAILABLE","message":"Upstream stream failed"}}})));
                return;
            }
            let native_id = session.provider_session_id.lock().await.clone().or(context.provider_session_id);
            if store && localmux_storage::save_response_mapping(&pool, localmux_storage::NewResponseMapping { owner_key_id: &owner_key_id, conversation_id: &context.conversation_id, response_id: &response_id, model_id: &context.model, adapter_id: context.adapter.id(), provider_session_id: native_id.as_deref(), previous_response_id: previous.as_deref(), created_at: created }).await.is_err() {
                yield Ok(event("response.failed", json!({"type":"response.failed","sequence_number":sequence,"response":{"id":response_id,"status":"failed","error":{"code":"LMX_INTERNAL_ERROR","message":"Session persistence failed"}}})));
                return;
            }
            yield Ok(event("response.output_text.done", json!({"type":"response.output_text.done","sequence_number":sequence,"item_id":message_id,"output_index":0,"content_index":0,"text":text})));
            sequence += 1;
            yield Ok(event("response.content_part.done", json!({"type":"response.content_part.done","sequence_number":sequence,"item_id":message_id,"output_index":0,"content_index":0,"part":{"type":"output_text","text":text,"annotations":[]}})));
            sequence += 1;
            yield Ok(event("response.output_item.done", json!({"type":"response.output_item.done","sequence_number":sequence,"output_index":0,"item":{"id":message_id,"type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":text,"annotations":[]}]}})));
            sequence += 1;
            yield Ok(event("response.completed", json!({"type":"response.completed","sequence_number":sequence,"response":response_object(&response_id, &message_id, created, &context.model, &text, previous.as_deref(), &metadata)})));
        };
        return Sse::new(events).into_response();
    }
    let result = if let Some(native_id) = context.provider_session_id.as_deref() {
        context.adapter.resume(normalized, native_id).await
    } else {
        context.adapter.invoke(normalized).await
    };
    let result = match result {
        Ok(result) => result,
        Err(AdapterError::Unsupported) if context.provider_session_id.is_some() => {
            return error(
                StatusCode::CONFLICT,
                "LMX_SESSION_RESUME_FAILED",
                "Upstream resume unsupported",
            )
        }
        Err(kind) => return adapter_error(kind),
    };
    let native_id = result
        .provider_session_id
        .as_deref()
        .or(context.provider_session_id.as_deref());
    if request.store.unwrap_or(true)
        && localmux_storage::save_response_mapping(
            &state.pool,
            localmux_storage::NewResponseMapping {
                owner_key_id: &key.id,
                conversation_id: &context.conversation_id,
                response_id: &response_id,
                model_id: &context.model,
                adapter_id: context.adapter.id(),
                provider_session_id: native_id,
                previous_response_id: request.previous_response_id.as_deref(),
                created_at: created,
            },
        )
        .await
        .is_err()
    {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "LMX_INTERNAL_ERROR",
            "Session persistence failed",
        );
    }
    Json(response_object(
        &response_id,
        &message_id,
        created,
        &context.model,
        &result.text,
        request.previous_response_id.as_deref(),
        &metadata,
    ))
    .into_response()
}
