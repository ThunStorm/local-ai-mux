use crate::{default_key_material, gateway_base_url, GatewayControl};
use futures_util::StreamExt;
use localmux_api_types::ChatRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};
use tauri::{ipc::Channel, State, WebviewWindow};
use tokio::sync::{oneshot, Mutex};

#[derive(Default)]
pub struct Control {
    state: Mutex<RunState>,
}
#[derive(Default)]
struct RunState {
    active: Option<Active>,
    pending_cancel: Option<(String, String)>,
}
struct Active {
    id: String,
    window: String,
    cancel: Option<oneshot::Sender<()>>,
}
impl Control {
    pub fn busy(&self) -> bool {
        self.state
            .try_lock()
            .map_or(true, |state| state.active.is_some())
    }
    async fn admit(
        &self,
        id: &str,
        window: &str,
        cancel: oneshot::Sender<()>,
    ) -> Result<bool, String> {
        let mut state = self.state.lock().await;
        if state.active.is_some() {
            return Err("LMX_CHAT_BUSY".into());
        }
        if state
            .pending_cancel
            .take()
            .is_some_and(|(request, owner)| request == id && owner == window)
        {
            return Ok(false);
        }
        state.active = Some(Active {
            id: id.into(),
            window: window.into(),
            cancel: Some(cancel),
        });
        Ok(true)
    }
    async fn cancel(&self, window: &str, id: &str) -> Result<(), String> {
        let mut state = self.state.lock().await;
        if state.active.is_none() {
            // One bounded, window-scoped tombstone handles Stop racing admission.
            state.pending_cancel = Some((id.into(), window.into()));
            return Ok(());
        }
        let Some(active) = state
            .active
            .as_mut()
            .filter(|active| active.id == id && active.window == window)
        else {
            return Err("LMX_REQUEST_NOT_ACTIVE".into());
        };
        if let Some(cancel) = active.cancel.take() {
            let _ = cancel.send(());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    request_id: String,
    sequence: u64,
    kind: &'static str,
    model: String,
    text: String,
    error_code: String,
    elapsed_ms: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    request_id: String,
    request: ChatRequest,
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(Duration::from_secs(125))
        .build()
        .map_err(|_| "LMX_CLIENT_UNAVAILABLE".into())
}
#[tauri::command]
pub async fn chat_models(control: State<'_, GatewayControl>) -> Result<Value, String> {
    if !crate::gateway_status(control.clone()).await? {
        return Err("LMX_API_STOPPED".into());
    }
    let (_, key) = default_key_material(&control)
        .await?
        .ok_or("LMX_KEY_MISSING")?;
    let response = client()?
        .get(format!(
            "http://127.0.0.1:{}/api/chat/models",
            localmux_gateway::DEFAULT_PORT
        ))
        .bearer_auth(key)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| "LMX_API_UNAVAILABLE")?;
    if !response.status().is_success() {
        return Err(status_code(response.status().as_u16()).into());
    }
    response
        .json()
        .await
        .map_err(|_| "LMX_INVALID_RESPONSE".into())
}
#[tauri::command]
pub async fn chat_send(
    window: WebviewWindow,
    control: State<'_, GatewayControl>,
    input: Input,
    events: Channel<Event>,
) -> Result<(), String> {
    if input.request_id.len() != 36
        || !input
            .request_id
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        return Err("LMX_INVALID_REQUEST_ID".into());
    }
    let bytes = serde_json::to_vec(&input.request).map_err(|_| "LMX_INVALID_REQUEST")?;
    if bytes.len() > 1024 * 1024 || !input.request.model.contains('/') {
        return Err("LMX_INVALID_REQUEST".into());
    }
    if !crate::gateway_status(control.clone()).await? {
        return Err("LMX_API_STOPPED".into());
    }
    let (_, key) = default_key_material(&control)
        .await?
        .ok_or("LMX_KEY_MISSING")?;
    let client = client()?;
    let (sender, cancel) = oneshot::channel();
    if !control
        .playground
        .admit(&input.request_id, window.label(), sender)
        .await?
    {
        let _ = events.send(Event {
            request_id: input.request_id,
            sequence: 1,
            kind: "stopped",
            model: input.request.model,
            text: String::new(),
            error_code: String::new(),
            elapsed_ms: 0,
        });
        return Ok(());
    }
    let started = Instant::now();
    let base = gateway_base_url();
    let mut sequence = 0;
    let mut emit = |kind: &'static str, text: &str, error_code: &str| {
        sequence += 1;
        events
            .send(Event {
                request_id: input.request_id.clone(),
                sequence,
                kind,
                model: input.request.model.clone(),
                text: text.into(),
                error_code: error_code.into(),
                elapsed_ms: started.elapsed().as_millis() as u64,
            })
            .map_err(|_| "LMX_CLIENT_GONE")
    };
    let result = match emit("started", "", "") {
        Ok(()) => tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(120), execute(&client, &base, &key, &input.request, |text| emit("delta", text, ""))) => match result {
                Ok(result) => result,
                Err(_) => Err("LMX_TIMEOUT"),
            },
            _ = cancel => Err("LMX_STOPPED"),
        },
        Err(error) => Err(error),
    };
    let (kind, code) = match result {
        Ok(()) => ("completed", ""),
        Err("LMX_STOPPED") => ("stopped", ""),
        Err(error) => ("failed", error),
    };
    // Drop the HTTP/SSE future before publishing a terminal event. A remote provider
    // may keep generating; this only guarantees cancellation of local reception.
    let mut state = control.playground.state.lock().await;
    if state
        .active
        .as_ref()
        .is_some_and(|active| active.id == input.request_id)
    {
        state.active = None;
    }
    drop(state);
    let _ = emit(kind, "", code);
    Ok(())
}
#[tauri::command]
pub async fn chat_cancel(
    window: WebviewWindow,
    control: State<'_, GatewayControl>,
    request_id: String,
) -> Result<(), String> {
    if request_id.len() != 36
        || !request_id
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        return Err("LMX_INVALID_REQUEST_ID".into());
    }
    control.playground.cancel(window.label(), &request_id).await
}
fn status_code(status: u16) -> &'static str {
    match status {
        401 => "LMX_AUTH_REQUIRED",
        403 => "LMX_PERMISSION_DENIED",
        404 => "LMX_MODEL_UNAVAILABLE",
        429 => "LMX_RATE_LIMITED",
        _ => "LMX_PROVIDER_UNAVAILABLE",
    }
}
fn safe_error(value: &Value, fallback: &'static str) -> &'static str {
    match value["error"]["code"].as_str() {
        Some("LMX_CONTEXT_LIMIT") => "LMX_CONTEXT_LIMIT",
        Some("LMX_REFUSAL") => "LMX_REFUSAL",
        Some("LMX_OUTPUT_TOO_LARGE") => "LMX_OUTPUT_TOO_LARGE",
        Some("LMX_STREAM_INTERRUPTED") => "LMX_STREAM_INTERRUPTED",
        Some("LMX_UNSUPPORTED_PARAMETER") => "LMX_UNSUPPORTED_PARAMETER",
        Some("LMX_UNSUPPORTED_MESSAGES") => "LMX_UNSUPPORTED_MESSAGES",
        Some("LMX_INVALID_REQUEST") => "LMX_INVALID_REQUEST",
        Some("LMX_QUOTA_EXHAUSTED") => "LMX_QUOTA_EXHAUSTED",
        Some("LMX_RATE_LIMITED") => "LMX_RATE_LIMITED",
        Some("LMX_PERMISSION_DENIED") => "LMX_PERMISSION_DENIED",
        Some("LMX_AUTH_REQUIRED") => "LMX_AUTH_REQUIRED",
        Some("LMX_MODEL_UNAVAILABLE") => "LMX_MODEL_UNAVAILABLE",
        Some("LMX_PROCESS_TIMEOUT") => "LMX_TIMEOUT",
        Some("LMX_PROVIDER_COOLDOWN") => "LMX_PROVIDER_COOLDOWN",
        Some("LMX_PROVIDER_UNAVAILABLE") => "LMX_PROVIDER_UNAVAILABLE",
        _ => fallback,
    }
}
fn terminal_error(value: &Value) -> Option<&'static str> {
    match value["choices"][0]["finish_reason"].as_str() {
        Some("length") => Some("LMX_OUTPUT_TRUNCATED"),
        Some("content_filter") => Some("LMX_REFUSAL"),
        _ if value["localmux"]["finish_reason"] == "cancelled"
            || value["choices"][0]["localmux"]["finish_reason"] == "cancelled" =>
        {
            Some("LMX_STREAM_INTERRUPTED")
        }
        _ => None,
    }
}
async fn execute(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    request: &ChatRequest,
    mut emit: impl FnMut(&str) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let response = client
        .post(format!("{base}/chat/completions"))
        .bearer_auth(key)
        .header("x-localmux-source", "playground")
        .json(request)
        .send()
        .await
        .map_err(|_| "LMX_API_UNAVAILABLE")?;
    if !response.status().is_success() {
        let fallback = status_code(response.status().as_u16());
        let value = response.json::<Value>().await.unwrap_or_default();
        return Err(safe_error(&value, fallback));
    }
    if !request.stream {
        let value = response
            .json::<Value>()
            .await
            .map_err(|_| "LMX_INVALID_RESPONSE")?;
        let text = value["choices"][0]["message"]["content"]
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or("LMX_EMPTY_RESPONSE")?;
        if text.len() > 4 * 1024 * 1024 {
            return Err("LMX_OUTPUT_TOO_LARGE");
        }
        emit(text)?;
        return terminal_error(&value).map_or(Ok(()), Err);
    }
    if !response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/event-stream"))
    {
        return Err("LMX_INVALID_RESPONSE");
    }
    let mut stream = response.bytes_stream();
    let mut decoder = Decoder::default();
    let mut total = 0;
    while let Some(chunk) =
        tokio::time::timeout(localmux_gateway::STREAM_IDLE_TIMEOUT, stream.next())
            .await
            .map_err(|_| "LMX_TIMEOUT")?
    {
        for data in decoder.push(&chunk.map_err(|_| "LMX_STREAM_INTERRUPTED")?)? {
            if data == "[DONE]" {
                return if total > 0 {
                    Ok(())
                } else {
                    Err("LMX_EMPTY_RESPONSE")
                };
            }
            let value: Value = serde_json::from_str(&data).map_err(|_| "LMX_INVALID_RESPONSE")?;
            if value.get("error").is_some() {
                return Err(safe_error(&value, "LMX_PROVIDER_UNAVAILABLE"));
            }
            if let Some(code) = terminal_error(&value) {
                return Err(code);
            }
            if let Some(text) = value["choices"][0]["delta"]["content"].as_str() {
                total += text.len();
                if total > 4 * 1024 * 1024 {
                    return Err("LMX_OUTPUT_TOO_LARGE");
                }
                if !text.is_empty() {
                    emit(text)?;
                }
            }
        }
    }
    Err("LMX_STREAM_INTERRUPTED")
}

#[derive(Default)]
struct Decoder {
    buffer: Vec<u8>,
}
impl Decoder {
    fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>, &'static str> {
        self.buffer.extend_from_slice(chunk);
        if self.buffer.len() > 1024 * 1024 {
            return Err("LMX_INVALID_RESPONSE");
        }
        let mut frames = Vec::new();
        loop {
            let lf = self
                .buffer
                .windows(2)
                .position(|v| v == b"\n\n")
                .map(|i| (i, 2));
            let crlf = self
                .buffer
                .windows(4)
                .position(|v| v == b"\r\n\r\n")
                .map(|i| (i, 4));
            let Some((end, size)) = [lf, crlf].into_iter().flatten().min_by_key(|(i, _)| *i) else {
                break;
            };
            let bytes: Vec<_> = self.buffer.drain(..end + size).collect();
            let text = std::str::from_utf8(&bytes[..end]).map_err(|_| "LMX_INVALID_RESPONSE")?;
            let data = text
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("data:")
                        .map(|v| v.strip_prefix(' ').unwrap_or(v))
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() {
                frames.push(data);
            }
        }
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sse_preserves_split_utf8_multiline_and_crlf() {
        let mut decoder = Decoder::default();
        let input = "data: {\"文字\":\r\ndata: \"你好\"}\r\n\r\ndata: [DONE]\n\n";
        let mut frames = Vec::new();
        for byte in input.as_bytes() {
            frames.extend(decoder.push(&[*byte]).unwrap());
        }
        assert_eq!(frames, vec!["{\"文字\":\n\"你好\"}", "[DONE]"]);
    }
    #[test]
    fn truncated_refused_and_cancelled_responses_do_not_enter_complete_history() {
        assert_eq!(
            terminal_error(&serde_json::json!({"choices":[{"finish_reason":"length"}]})),
            Some("LMX_OUTPUT_TRUNCATED")
        );
        assert_eq!(
            terminal_error(&serde_json::json!({"choices":[{"finish_reason":"content_filter"}]})),
            Some("LMX_REFUSAL")
        );
        assert_eq!(
            terminal_error(
                &serde_json::json!({"choices":[{"finish_reason":null,"localmux":{"finish_reason":"cancelled"}}]})
            ),
            Some("LMX_STREAM_INTERRUPTED")
        );
        assert_eq!(
            terminal_error(&serde_json::json!({"choices":[{"finish_reason":null}]})),
            None
        );
    }
    #[test]
    fn errors_never_expose_gateway_message_or_unknown_codes() {
        for (code, expected) in [
            ("LMX_PROCESS_TIMEOUT", "LMX_TIMEOUT"),
            ("LMX_PROVIDER_COOLDOWN", "LMX_PROVIDER_COOLDOWN"),
            ("LMX_AUTH_REQUIRED", "LMX_AUTH_REQUIRED"),
        ] {
            assert_eq!(
                safe_error(
                    &serde_json::json!({"error":{"code":code,"message":"private detail"}}),
                    "fallback"
                ),
                expected
            );
        }
        let error = serde_json::json!({"error":{"code":"unexpected-secret","message":"private prompt/key"}});
        assert_eq!(
            safe_error(&error, "LMX_PROVIDER_UNAVAILABLE"),
            "LMX_PROVIDER_UNAVAILABLE"
        );
    }
    #[tokio::test]
    async fn cancellation_is_bound_to_window_and_request() {
        let control = Control::default();
        let (sender, receiver) = oneshot::channel();
        control.state.lock().await.active = Some(Active {
            id: "request".into(),
            window: "main".into(),
            cancel: Some(sender),
        });
        assert!(control.cancel("other", "request").await.is_err());
        assert!(control.cancel("main", "stale").await.is_err());
        assert!(control.busy());
        control.cancel("main", "request").await.unwrap();
        receiver.await.unwrap();
        assert!(control.busy()); // Remains busy until the HTTP future is dropped.
    }
    #[tokio::test]
    async fn early_stop_cannot_start_inference_or_cancel_another_window() {
        let control = Control::default();
        control.cancel("main", "before-start").await.unwrap();
        let (sender, _) = oneshot::channel();
        assert!(!control.admit("before-start", "main", sender).await.unwrap());
        assert!(!control.busy());
        control.cancel("other", "same-id").await.unwrap();
        let (sender, _) = oneshot::channel();
        assert!(control.admit("same-id", "main", sender).await.unwrap());
        let (sender, _) = oneshot::channel();
        assert!(control.admit("different", "main", sender).await.is_err());
    }
    #[tokio::test]
    async fn bridge_consumes_actual_mock_gateway_json_and_sse() {
        let state =
            localmux_gateway::GatewayState::mock(localmux_storage::open_memory().await.unwrap());
        let (_, key) = state.create_key(&["chat:invoke"]).await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!(
            "http://127.0.0.1:{}/v1",
            listener.local_addr().unwrap().port()
        );
        let server_state = state.clone();
        let server = tokio::spawn(async move {
            localmux_gateway::serve_on(listener, server_state)
                .await
                .unwrap();
        });
        for stream in [false, true] {
            let request: ChatRequest = serde_json::from_value(serde_json::json!({"model":"mock/echo","messages":[{"role":"user","content":"offline 中文"}],"stream":stream})).unwrap();
            let mut text = String::new();
            execute(&client().unwrap(), &base, &key, &request, |delta| {
                text.push_str(delta);
                Ok(())
            })
            .await
            .unwrap();
            assert_eq!(text, "offline 中文");
        }
        assert_eq!(state.active_tasks(), 0);
        server.abort();
    }
    #[tokio::test]
    async fn missing_done_is_not_reported_as_completed() {
        use axum::{body::Body, http::Response, routing::post, Router};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let app = Router::new().route(
            "/chat/completions",
            post(|| async {
                Response::builder()
                    .header("content-type", "text/event-stream")
                    .body(Body::from(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
                    ))
                    .unwrap()
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let request:ChatRequest=serde_json::from_value(serde_json::json!({"model":"fixture/model","messages":[{"role":"user","content":"offline"}],"stream":true})).unwrap();
        let mut text = String::new();
        assert_eq!(
            execute(&client().unwrap(), &base, "synthetic", &request, |delta| {
                text.push_str(delta);
                Ok(())
            })
            .await,
            Err("LMX_STREAM_INTERRUPTED")
        );
        assert_eq!(text, "partial");
        server.abort();
    }
    #[tokio::test]
    async fn dropping_reception_releases_the_server_stream() {
        use axum::{
            response::sse::{Event as SseEvent, Sse},
            routing::post,
            Router,
        };
        use std::{
            convert::Infallible,
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc,
            },
        };
        struct Release(Arc<AtomicBool>);
        impl Drop for Release {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let released = Arc::new(AtomicBool::new(false));
        let for_route = released.clone();
        let app = Router::new().route(
            "/chat/completions",
            post(move || {
                let release = Release(for_route.clone());
                async move {
                    let first = futures_util::stream::once(async {
                        Ok::<_, Infallible>(
                            SseEvent::default()
                                .data("{\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}"),
                        )
                    });
                    let stream = first
                        .chain(futures_util::stream::pending())
                        .map(move |item| {
                            let _ = &release;
                            item
                        });
                    Sse::new(stream)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let seen = Arc::new(AtomicBool::new(false));
        let for_client = seen.clone();
        let request:ChatRequest=serde_json::from_value(serde_json::json!({"model":"fixture/model","messages":[{"role":"user","content":"offline"}],"stream":true})).unwrap();
        let task = tokio::spawn(async move {
            execute(&client().unwrap(), &base, "synthetic", &request, |_| {
                for_client.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !seen.load(Ordering::SeqCst) {
                tokio::task::yield_now().await
            }
        })
        .await
        .unwrap();
        task.abort();
        let _ = task.await;
        tokio::time::timeout(Duration::from_secs(2), async {
            while !released.load(Ordering::SeqCst) {
                tokio::task::yield_now().await
            }
        })
        .await
        .unwrap();
        server.abort();
    }
}
