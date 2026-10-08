//! Call audit and opt-in dialogue capture, independent of provider session continuity.
use super::{now, ResponseMetadata};
use axum::{body::Body, response::Response};
use futures_util::StreamExt;
use localmux_adapter_sdk::FinishReason;
use localmux_storage::CallRecord;
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub(crate) struct Call(
    Option<Arc<Mutex<CallRecord>>>,
    SqlitePool,
    Arc<tokio::sync::Mutex<Option<ContentCapture>>>,
);
struct ContentCapture {
    generation: i64,
    raw_answer: String,
}
fn redact_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => localmux_telemetry::redact_credentials(text).into(),
        serde_json::Value::Array(items) => items.iter().map(redact_value).collect(),
        serde_json::Value::Object(items) => items
            .iter()
            .map(|(key, value)| {
                let sensitive = matches!(
                    key.to_ascii_lowercase().replace(['_', '-'], "").as_str(),
                    "authorization"
                        | "apikey"
                        | "accesstoken"
                        | "refreshtoken"
                        | "password"
                        | "cookie"
                );
                (
                    key.clone(),
                    if sensitive {
                        "[REDACTED]".into()
                    } else {
                        redact_value(value)
                    },
                )
            })
            .collect(),
        _ => value.clone(),
    }
}
impl Call {
    pub async fn request(&self, messages: &serde_json::Value) -> anyhow::Result<()> {
        let Some(record) = &self.0 else {
            return Ok(());
        };
        let id = record.lock().unwrap().id.clone();
        let generation =
            localmux_storage::start_content(&self.1, &id, &redact_value(messages).to_string())
                .await?;
        if let Some(generation) = generation {
            *self.2.lock().await = Some(ContentCapture {
                generation,
                raw_answer: String::new(),
            });
        }
        Ok(())
    }
    pub async fn answer(&self, delta: &str) -> anyhow::Result<()> {
        let mut capture = self.2.lock().await;
        let Some(content) = capture.as_mut() else {
            return Ok(());
        };
        content.raw_answer.push_str(delta);
        let id = self.0.as_ref().unwrap().lock().unwrap().id.clone();
        // ponytail: re-redact snapshots for cross-chunk credentials; move to a streaming sanitizer only if measured long-output latency warrants it.
        let safe = localmux_telemetry::redact_credentials(&content.raw_answer);
        if !localmux_storage::update_content(&self.1, &id, content.generation, &safe).await? {
            *capture = None;
        }
        Ok(())
    }
    fn terminal(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|record| record.lock().unwrap().status != "in_progress")
    }
    pub fn target(&self, model: &str, adapter: &str, response: &str, conversation: Option<&str>) {
        if let Some(record) = &self.0 {
            let mut record = record.lock().unwrap();
            record.resolved_model = Some(safe_model(model));
            record.adapter_id = Some(safe_model(adapter));
            if !response.is_empty() {
                record.response_id = Some(response.to_owned());
            }
            record.conversation_id = conversation.map(str::to_owned);
        }
    }
    pub fn metadata(&self, metadata: &ResponseMetadata) {
        if let Some(record) = &self.0 {
            let mut record = record.lock().unwrap();
            record.finish_reason = Some(
                match metadata.finish_reason {
                    FinishReason::Stop => "stop",
                    FinishReason::Length => "length",
                    FinishReason::ContentFilter => "content_filter",
                    FinishReason::Cancelled => "cancelled",
                    FinishReason::Unknown => "unknown",
                }
                .into(),
            );
            record.actual_model = metadata.actual_model.as_deref().map(safe_model);
            if let Some(usage) = &metadata.usage {
                record.input_tokens = usage.input_tokens.and_then(|n| i64::try_from(n).ok());
                record.output_tokens = usage.output_tokens.and_then(|n| i64::try_from(n).ok());
                record.total_tokens = usage.total_tokens.and_then(|n| i64::try_from(n).ok());
            }
        }
    }
    pub fn result(&self, metadata: &ResponseMetadata) {
        self.metadata(metadata);
        if let Some(record) = &self.0 {
            record.lock().unwrap().status = match metadata.finish_reason {
                FinishReason::Length => "truncated",
                FinishReason::ContentFilter => "filtered",
                FinishReason::Cancelled => "cancelled",
                _ => "completed",
            }
            .into();
        }
    }
    pub fn fail(&self, code: &str) {
        if let Some(record) = &self.0 {
            let mut record = record.lock().unwrap();
            record.status = match code {
                "LMX_PROCESS_TIMEOUT" => "timeout",
                "LMX_STREAM_INTERRUPTED" => "interrupted",
                _ => "failed",
            }
            .into();
            // Never persist upstream free text or arbitrary caller-provided strings.
            record.error_code = Some(
                if code.starts_with("LMX_")
                    && code.len() < 80
                    && code.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
                {
                    code
                } else {
                    "LMX_INTERNAL_ERROR"
                }
                .into(),
            );
        }
    }
}
fn safe_model(value: &str) -> String {
    if !value.is_empty()
        && value.len() <= 160
        && !value.starts_with("lmx_")
        && !value.starts_with("sk-")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_.:-".contains(&b))
    {
        value.into()
    } else {
        "[invalid model]".into()
    }
}

pub(crate) struct CallGuard {
    call: Call,
    pool: SqlitePool,
    start: Instant,
    persisted: bool,
}
impl CallGuard {
    pub async fn start(
        pool: &SqlitePool,
        owner: Option<&str>,
        source: &str,
        endpoint: &str,
        model: &str,
        streamed: bool,
        store: bool,
    ) -> anyhow::Result<Self> {
        let call = if store {
            let record = CallRecord {
                sequence: 0,
                id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                owner_key_id: owner.map(str::to_owned),
                source: source.into(),
                endpoint: endpoint.into(),
                requested_model: safe_model(model),
                resolved_model: None,
                adapter_id: None,
                response_id: None,
                conversation_id: None,
                streamed,
                started_at: now(),
                finished_at: None,
                elapsed_ms: None,
                status: "in_progress".into(),
                error_code: None,
                finish_reason: None,
                actual_model: None,
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
            };
            Call(
                Some(Arc::new(Mutex::new(record))),
                pool.clone(),
                Arc::new(tokio::sync::Mutex::new(None)),
            )
        } else {
            Call(None, pool.clone(), Arc::new(tokio::sync::Mutex::new(None)))
        };
        let guard = Self {
            call,
            pool: pool.clone(),
            start: Instant::now(),
            persisted: false,
        };
        if let Some(record) = guard.snapshot() {
            localmux_storage::start_call(pool, &record).await?;
        }
        Ok(guard)
    }
    pub fn call(&self) -> Call {
        self.call.clone()
    }
    fn snapshot(&self) -> Option<CallRecord> {
        self.call.0.as_ref().map(|record| {
            let mut record = record.lock().unwrap().clone();
            if record.status == "in_progress" {
                record.status = "cancelled".into();
            }
            record.finished_at = Some(now());
            record.elapsed_ms = Some(self.start.elapsed().as_millis().min(i64::MAX as u128) as i64);
            record
        })
    }
    pub async fn persist(&mut self) -> anyhow::Result<()> {
        if let Some(record) = self.snapshot() {
            if let Err(error) = localmux_storage::finish_call(&self.pool, &record).await {
                self.call.fail("LMX_INTERNAL_ERROR");
                return Err(error);
            }
        }
        self.persisted = true;
        Ok(())
    }
    pub async fn response(mut self, response: Response) -> Response {
        if self.call.0.is_none() {
            self.persisted = true;
            return response;
        }
        if !response.status().is_success() {
            let (parts, body) = response.into_parts();
            let bytes = axum::body::to_bytes(body, 64 * 1024)
                .await
                .unwrap_or_default();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
            self.call.fail(
                value["error"]["code"]
                    .as_str()
                    .unwrap_or("LMX_INTERNAL_ERROR"),
            );
            if self.persist().await.is_err() {
                return persistence_error();
            }
            return Response::from_parts(parts, Body::from(bytes));
        }
        if response.headers().get("content-type").is_some_and(|v| {
            v.to_str()
                .unwrap_or_default()
                .starts_with("text/event-stream")
        }) {
            let (parts, body) = response.into_parts();
            // Own the guard outside the inner generator, including an unpolled body.
            let mut data = body.into_data_stream();
            let stream = async_stream::stream! {
                let mut guard = self;
                while let Some(frame) = data.next().await {
                    if frame.is_err() { guard.call.fail("LMX_STREAM_INTERRUPTED"); }
                    // Persist a known terminal result before forwarding its terminal frame.
                    // SDKs may stop polling immediately after completed / [DONE].
                    if !guard.persisted && guard.call.terminal() && guard.persist().await.is_err() {
                        yield Err(axum::Error::new(std::io::Error::other("Call record persistence failed")));
                        return;
                    }
                    yield frame;
                }
                if !guard.persisted && guard.persist().await.is_err() {
                    yield Err(axum::Error::new(std::io::Error::other("Call record persistence failed")));
                }
            };
            Response::from_parts(parts, Body::from_stream(stream))
        } else {
            if self.persist().await.is_err() {
                return persistence_error();
            }
            response
        }
    }
}
impl Drop for CallGuard {
    fn drop(&mut self) {
        if self.persisted {
            return;
        }
        if let (Some(record), Ok(runtime)) =
            (self.snapshot(), tokio::runtime::Handle::try_current())
        {
            let pool = self.pool.clone();
            runtime.spawn(async move {
                if localmux_storage::finish_call(&pool, &record).await.is_err() {
                    eprintln!("LocalMux call record persistence failed");
                }
            });
        }
    }
}
pub(crate) fn persistence_error() -> Response {
    super::error(
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "LMX_INTERNAL_ERROR",
        "Call record persistence failed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn full_unicode_dialogue_is_not_silently_truncated() {
        let pool = localmux_storage::open_memory().await.unwrap();
        localmux_storage::set_recording(&pool, true).await.unwrap();
        let mut guard = CallGuard::start(&pool, None, "api", "chat", "mock/echo", true, true)
            .await
            .unwrap();
        let input = "完整原文\n".repeat(5000);
        let output = "完整模型回答😀\n".repeat(8000);
        guard
            .call()
            .request(&serde_json::json!([{"role":"user","content":input}]))
            .await
            .unwrap();
        guard.call().answer(&output).await.unwrap();
        guard.call().result(&ResponseMetadata::default());
        guard.persist().await.unwrap();
        let id = guard.snapshot().unwrap().id;
        let body = localmux_storage::call_content(&pool, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(body.messages[0]["content"], input);
        assert_eq!(body.answer, output);
        assert_eq!(body.state, "complete");
    }

    #[test]
    fn nested_and_camel_case_credentials_are_redacted_without_losing_other_dialogue() {
        let safe = redact_value(
            &serde_json::json!([{ "role":"user", "content":{ "accessToken":"HIDDEN_ACCESS", "refresh-token":"HIDDEN_REFRESH", "apiKey":"HIDDEN_KEY", "nested":["user@example.com", "Bearer HIDDEN_BEARER"], "normal":"保留普通内容" } }]),
        );
        let encoded = safe.to_string();
        for value in [
            "HIDDEN_ACCESS",
            "HIDDEN_REFRESH",
            "HIDDEN_KEY",
            "HIDDEN_BEARER",
        ] {
            assert!(!encoded.contains(value));
        }
        assert!(encoded.contains("user@example.com"));
        assert!(encoded.contains("保留普通内容"));
    }

    #[tokio::test]
    async fn terminal_write_failure_cannot_be_retried_as_a_success() {
        let pool = localmux_storage::open_memory().await.unwrap();
        let mut guard = CallGuard::start(&pool, None, "api", "chat", "mock/echo", false, true)
            .await
            .unwrap();
        guard.call().result(&ResponseMetadata::default());
        sqlx::query("CREATE TRIGGER refuse_call_finish BEFORE UPDATE ON call_records BEGIN SELECT RAISE(FAIL, 'fixture'); END;")
            .execute(&pool).await.unwrap();
        assert!(guard.persist().await.is_err());
        sqlx::query("DROP TRIGGER refuse_call_finish")
            .execute(&pool)
            .await
            .unwrap();
        guard.persist().await.unwrap();
        let record = localmux_storage::list_calls(&pool, &Default::default())
            .await
            .unwrap()
            .remove(0);
        assert_eq!(record.status, "failed");
        assert_eq!(record.error_code.as_deref(), Some("LMX_INTERNAL_ERROR"));
    }
}
