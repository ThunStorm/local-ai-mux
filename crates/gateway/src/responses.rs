use super::{adapter_error, authorized, error, now, GatewayState};
use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures_util::StreamExt;
use localmux_adapter_sdk::{
    Adapter, AdapterError, FinishReason, NormalizedRequest, NormalizedResponse, ResponseMetadata,
    StreamEvent, TimeoutLayer, TokenUsage,
};
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
    store: bool,
    call: &super::recording::Call,
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
        call.target(
            &mapping.model_id,
            &mapping.adapter_id,
            "",
            Some(&mapping.id),
        );
        if !mapping.resume_supported || mapping.provider_session_id.is_none() {
            return Err(Box::new(error(
                StatusCode::CONFLICT,
                "LMX_SESSION_RESUME_FAILED",
                "Upstream model does not support resume",
            )));
        }
        let virtual_route = matches!(requested, "free" | "cheap" | "balanced" | "best");
        if !virtual_route && requested != mapping.model_id {
            if store {
                let _ = localmux_storage::record_run_event(
                    &state.pool,
                    &mapping.id,
                    "ROUTE_CHANGE_REJECTED",
                    &json!({"requested_model":requested}).to_string(),
                    now(),
                )
                .await;
            }
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
        if let Some(response) = super::connection_cooldown(state, adapter.id()) {
            return Err(Box::new(response));
        }
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
            if store {
                let _ = localmux_storage::record_run_event(
                    &state.pool,
                    &mapping.id,
                    "STICKY_ROUTE_UNAVAILABLE",
                    &json!({"reason":reason.to_string()}).to_string(),
                    now(),
                )
                .await;
            }
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
    if let Some(response) = super::connection_cooldown(state, adapter.id()) {
        return Err(Box::new(response));
    }
    if matches!(
        candidate[0].health,
        HealthStatus::AuthRequired
            | HealthStatus::Offline
            | HealthStatus::Unsupported
            | HealthStatus::QuotaExhausted
    ) {
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
    ids: (&str, &str),
    created: i64,
    model: &str,
    response: &NormalizedResponse,
    previous: Option<&str>,
    metadata: &Value,
    budget: &super::execution::ExecutionBudget,
) -> Value {
    let (id, message_id) = ids;
    let result = &response.metadata;
    let text = &response.text;
    let incomplete = matches!(
        result.finish_reason,
        FinishReason::Length | FinishReason::Cancelled | FinishReason::ContentFilter
    );
    let usage = result.usage.clone().and_then(TokenUsage::validated).map(|u| {
        let mut value = json!({"input_tokens":u.input_tokens,"output_tokens":u.output_tokens,"total_tokens":u.total_tokens});
        if let Some(cached) = u.cached_input_tokens {
            value["input_tokens_details"] = json!({"cached_tokens":cached});
        }
        value
    });
    json!({"id":id,"object":"response","created_at":created,"status":if incomplete { "incomplete" } else { "completed" },"error":null,"incomplete_details":if incomplete { json!({"reason": match result.finish_reason { FinishReason::Length => "max_output_tokens", FinishReason::ContentFilter => "content_filter", _ => "cancelled" }}) } else { Value::Null },
        "model":model,"previous_response_id":previous,"metadata":metadata,
        "output":[{"id":message_id,"type":"message","status":if incomplete { "incomplete" } else { "completed" },"role":"assistant","content":[{"type":"output_text","text":text,"annotations":[]}]}],
        "output_text":text,"usage":usage,"localmux":super::wire_metadata(result,id,model,None,budget)})
}
fn event(name: &str, data: Value) -> Event {
    Event::default().event(name).data(data.to_string())
}

pub async fn create(
    State(state): State<GatewayState>,
    Extension(budget): Extension<super::execution::ExecutionBudget>,
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
    let guard = match super::recording::CallGuard::start(
        &state.pool,
        Some(&key.id),
        "api",
        "responses",
        &request.model,
        request.stream,
        request.store.unwrap_or(true),
    )
    .await
    {
        Ok(guard) => guard.with_budget(budget),
        Err(_) => return super::recording::persistence_error(),
    };
    let call = guard.call();
    call.budget().metrics.lock().unwrap().request_bytes = Some(body.len() as u64);
    let messages = if request.input.is_string() {
        json!([{"role":"user","content":request.input}])
    } else {
        request.input.clone()
    };
    if call.request(&messages).await.is_err() {
        call.fail("LMX_INTERNAL_ERROR");
        return guard.response(super::recording::persistence_error()).await;
    }
    let response = create_inner(state, key, request, call).await;
    guard.response(response).await
}
async fn create_inner(
    state: GatewayState,
    key: localmux_storage::ApiKeyRecord,
    request: localmux_api_types::ResponsesRequest,
    call: super::recording::Call,
) -> Response {
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
        request.store.unwrap_or(true),
        &call,
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
    call.target(
        &context.model,
        context.adapter.id(),
        &response_id,
        Some(&context.conversation_id),
    );
    let normalized = NormalizedRequest {
        cli_idle_timeout: call.budget().cli_idle_timeout,
        model: context.model.clone(),
        prompt,
        messages: Vec::new(),
        parameters: Default::default(),
    };
    {
        let budget = call.budget();
        let mut metrics = budget.metrics.lock().unwrap();
        metrics.auth_route_ms = Some(budget.elapsed_ms());
        metrics.route_kind = Some(
            if matches!(
                request.model.as_str(),
                "free" | "cheap" | "balanced" | "best"
            ) {
                "virtual_route"
            } else {
                "explicit_model"
            }
            .into(),
        );
    }
    let permit = match state
        .acquire_agent(context.adapter.as_ref(), &call.budget())
        .await
    {
        Ok(permit) => permit,
        Err(response) => return *response,
    };
    let deadline = call.budget().deadline;
    let stream_idle = if context.adapter.kind() == localmux_domain::AdapterKind::Agent {
        call.budget().cli_idle_timeout
    } else {
        super::STREAM_IDLE_TIMEOUT
    };
    let adapter_run = state.begin_adapter(context.adapter.id());
    let adapter_timer = call.budget().measure(super::execution::Phase::Adapter);
    if request.stream {
        let startup_timer = call.budget().measure(super::execution::Phase::Startup);
        let session = match tokio::time::timeout_at(
            call.budget().child_deadline(Duration::from_secs(30)),
            context
                .adapter
                .invoke_session_events(normalized, context.provider_session_id.as_deref()),
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
            Ok(Err(kind)) => {
                call.budget().record_timeout(&kind);
                super::breaker_error(&state, context.adapter.id(), &kind);
                return adapter_error(kind);
            }
            Err(_) => {
                return call
                    .budget()
                    .timeout_at(TimeoutLayer::GatewayStartup, Duration::from_secs(30))
            }
        };
        drop(startup_timer);
        let pool = state.pool.clone();
        let stream_state = state.clone();
        let owner_key_id = key.id.clone();
        let previous = request.previous_response_id;
        let store = request.store.unwrap_or(true);
        let events = async_stream::stream! {
            let _permit = permit;
            let mut provider_stream = session.stream;
            let mut text = String::new();
            yield Ok::<Event, Infallible>(event("response.created", json!({"type":"response.created","sequence_number":call.budget().next_sequence(),"response":{"id":response_id,"object":"response","created_at":created,"status":"in_progress","model":context.model,"output":[]}})));
            yield Ok(event("response.output_item.added", json!({"type":"response.output_item.added","sequence_number":call.budget().next_sequence(),"output_index":0,"item":{"id":message_id,"type":"message","status":"in_progress","role":"assistant","content":[]}})));
            yield Ok(event("response.content_part.added", json!({"type":"response.content_part.added","sequence_number":call.budget().next_sequence(),"item_id":message_id,"output_index":0,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}})));
            let mut failure = None;
            let mut result = ResponseMetadata::default();
            let mut completed = false;
            loop {
                let next_deadline = (tokio::time::Instant::now() + stream_idle).min(deadline);
                let next = match tokio::time::timeout_at(next_deadline, provider_stream.next()).await {
                    Ok(Some(next)) => next,
                    Ok(None) => { if completed { stream_state.breaker.success(context.adapter.id()); } else { failure = Some("LMX_STREAM_INTERRUPTED"); } break; },
                    Err(_) => {
                        call.budget().record_deadline(TimeoutLayer::GatewayStreamIdle, stream_idle);
                        stream_state.breaker.open(context.adapter.id(), Instant::now(), Duration::from_secs(30));
                        failure = Some("LMX_PROCESS_TIMEOUT"); break;
                    },
                };
                match next {
                    Ok(StreamEvent::Delta(delta)) if !completed => {
                        call.first_output(&delta);
                        if text.len() + delta.len() > 8 * 1024 * 1024 { failure = Some("LMX_OUTPUT_TOO_LARGE"); break; }
                        if call.answer(&delta).await.is_err() { failure = Some("LMX_INTERNAL_ERROR"); break; }
                        text.push_str(&delta);
                        yield Ok(event("response.output_text.delta", json!({"type":"response.output_text.delta","sequence_number":call.budget().next_sequence(),"item_id":message_id,"output_index":0,"content_index":0,"delta":delta})));
                                },
                    Ok(StreamEvent::Usage(usage)) if !completed => { result.usage = usage.validated(); call.metadata(&result); },
                    Ok(StreamEvent::Completed(mut reported)) if !completed => { reported = reported.validated(); reported.usage = reported.usage.or(result.usage.take()); result = reported; call.metadata(&result); completed = true; },
                    Ok(_) => { failure = Some("LMX_INVALID_RESPONSE"); break; },
                    Err(kind) => {
                        call.budget().record_timeout(&kind);
                        super::breaker_error(&stream_state, context.adapter.id(), &kind);
                        failure = Some(super::adapter_error_details(&kind).1); break;
                    },
                }
            }
            drop(adapter_timer);
            drop(adapter_run);
            if let Some(code) = failure {
                call.fail(code);
                yield Ok(event("response.failed", json!({"type":"response.failed","sequence_number":call.budget().next_sequence(),"response":{"id":response_id,"status":"failed","error":{"code":code,"message":"Upstream stream failed","request_id":call.budget().request_id,"param":null,"limiting_layer":call.budget().timeout_limiting_layer(),"timeout_layer":call.budget().snapshot().timeout_layer,"timeout_limit_ms":call.budget().snapshot().timeout_limit_ms,"reason_code":call.budget().snapshot().reason_code,"retryable":false}}})));
                return;
            }
            let native_id = session.provider_session_id.lock().await.clone().or(context.provider_session_id);
            if store && !matches!(result.finish_reason, FinishReason::Length | FinishReason::Cancelled | FinishReason::ContentFilter) && localmux_storage::save_response_mapping(&pool, localmux_storage::NewResponseMapping { owner_key_id: &owner_key_id, conversation_id: &context.conversation_id, response_id: &response_id, model_id: &context.model, adapter_id: context.adapter.id(), provider_session_id: native_id.as_deref(), previous_response_id: previous.as_deref(), created_at: created }).await.is_err() {
                call.fail("LMX_INTERNAL_ERROR");
                yield Ok(event("response.failed", json!({"type":"response.failed","sequence_number":call.budget().next_sequence(),"response":{"id":response_id,"status":"failed","error":{"code":"LMX_INTERNAL_ERROR","message":"Session persistence failed"}}})));
                return;
            }
            call.result(&result);
            yield Ok(event("response.output_text.done", json!({"type":"response.output_text.done","sequence_number":call.budget().next_sequence(),"item_id":message_id,"output_index":0,"content_index":0,"text":text})));
            yield Ok(event("response.content_part.done", json!({"type":"response.content_part.done","sequence_number":call.budget().next_sequence(),"item_id":message_id,"output_index":0,"content_index":0,"part":{"type":"output_text","text":text,"annotations":[]}})));
            yield Ok(event("response.output_item.done", json!({"type":"response.output_item.done","sequence_number":call.budget().next_sequence(),"output_index":0,"item":{"id":message_id,"type":"message","status":if matches!(result.finish_reason, FinishReason::Length | FinishReason::Cancelled | FinishReason::ContentFilter) { "incomplete" } else { "completed" },"role":"assistant","content":[{"type":"output_text","text":text,"annotations":[]}]}})));
            let object = response_object((&response_id, &message_id), created, &context.model, &NormalizedResponse { text, provider_session_id: None, metadata: result }, previous.as_deref(), &metadata, &call.budget());
            let terminal = if object["status"] == "incomplete" { "response.incomplete" } else { "response.completed" };
            yield Ok(event(terminal, json!({"type":terminal,"sequence_number":call.budget().next_sequence(),"response":object})));
        };
        return Sse::new(events)
            .keep_alive(axum::response::sse::KeepAlive::default())
            .into_response();
    }
    let result = match tokio::time::timeout_at(deadline, async {
        if let Some(native_id) = context.provider_session_id.as_deref() {
            context.adapter.resume(normalized, native_id).await
        } else {
            context.adapter.invoke(normalized).await
        }
    })
    .await
    {
        Ok(result) => result,
        Err(_) => return call.budget().timeout_response(),
    };
    let mut result = match result {
        Ok(result) => result,
        Err(AdapterError::Unsupported) if context.provider_session_id.is_some() => {
            return error(
                StatusCode::CONFLICT,
                "LMX_SESSION_RESUME_FAILED",
                "Upstream resume unsupported",
            )
        }
        Err(kind) => {
            call.budget().record_timeout(&kind);
            super::breaker_error(&state, context.adapter.id(), &kind);
            return adapter_error(kind);
        }
    };
    if result.text.len() > 8 * 1024 * 1024 {
        return adapter_error(AdapterError::OutputTooLarge);
    }
    result.metadata = result.metadata.validated();
    drop(adapter_timer);
    drop(adapter_run);
    let native_id = result
        .provider_session_id
        .as_deref()
        .or(context.provider_session_id.as_deref());
    if request.store.unwrap_or(true)
        && !matches!(
            result.metadata.finish_reason,
            FinishReason::Length | FinishReason::Cancelled | FinishReason::ContentFilter
        )
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
    if call.answer(&result.text).await.is_err() {
        return super::recording::persistence_error();
    }
    call.result(&result.metadata);
    Json(response_object(
        (&response_id, &message_id),
        created,
        &context.model,
        &result,
        request.previous_response_id.as_deref(),
        &metadata,
        &call.budget(),
    ))
    .into_response()
}
