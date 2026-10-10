//! One monotonic budget from HTTP admission, including queue and response consumption.
use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use futures_util::StreamExt;
use tokio::time::Instant;

#[derive(Clone)]
pub(crate) struct ExecutionBudget {
    pub accepted: Instant,
    pub deadline: Instant,
    pub request_id: String,
    pub response_id: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    pub response_sequence: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub metrics: std::sync::Arc<std::sync::Mutex<localmux_storage::CallDiagnostics>>,
}
impl Default for ExecutionBudget {
    fn default() -> Self {
        let accepted = Instant::now();
        Self {
            accepted,
            deadline: accepted + super::REQUEST_TIMEOUT,
            request_id: format!("req_{}", uuid::Uuid::new_v4().simple()),
            response_id: Default::default(),
            response_sequence: Default::default(),
            metrics: std::sync::Arc::new(std::sync::Mutex::new(
                localmux_storage::CallDiagnostics {
                    accepted_at: Some(super::now()),
                    upstream_cancel_verified: Some("UNKNOWN".into()),
                    ..Default::default()
                },
            )),
        }
    }
}
impl ExecutionBudget {
    pub fn next_sequence(&self) -> u64 {
        self.response_sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
    pub fn elapsed_ms(&self) -> u64 {
        self.accepted.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    pub fn measure(&self, phase: Phase) -> PhaseTimer {
        PhaseTimer(self.clone(), Instant::now(), phase)
    }
    pub fn snapshot(&self) -> localmux_storage::CallDiagnostics {
        let mut value = self.metrics.lock().unwrap().clone();
        value.total_elapsed_ms = Some(self.elapsed_ms());
        value
    }
    pub fn child_deadline(&self, limit: std::time::Duration) -> Instant {
        self.deadline.min(Instant::now() + limit)
    }
    pub fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }
    pub fn timeout_response(&self) -> Response {
        self.metrics.lock().unwrap().reason_code = Some("overall_deadline".into());
        super::error(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            "LMX_PROCESS_TIMEOUT",
            "Request deadline exceeded",
        )
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Queue,
    Interval,
    Startup,
    Adapter,
    Finalize,
}
pub(crate) struct PhaseTimer(ExecutionBudget, Instant, Phase);
impl Drop for PhaseTimer {
    fn drop(&mut self) {
        let ms = self.1.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let mut value = self.0.metrics.lock().unwrap();
        match self.2 {
            Phase::Queue => value.queue_wait_ms = Some(ms),
            Phase::Interval => value.interval_wait_ms = Some(ms),
            Phase::Startup => value.adapter_startup_ms = Some(ms),
            Phase::Adapter => value.adapter_ms = Some(ms),
            Phase::Finalize => value.response_finalize_ms = Some(ms),
        }
    }
}

pub(crate) async fn admit(
    axum::extract::State(state): axum::extract::State<super::GatewayState>,
    mut request: Request,
    next: Next,
) -> Response {
    let budget = ExecutionBudget::default();
    let responses = request.uri().path() == "/v1/responses";
    let run = (request.method() == axum::http::Method::POST).then(|| state.begin_run());
    request.extensions_mut().insert(budget.clone());
    let mut response = match tokio::time::timeout_at(budget.deadline, next.run(request)).await {
        Ok(response) => response,
        Err(_) => budget.timeout_response(),
    };
    if budget.expired() && response.status().is_success() {
        response = budget.timeout_response();
    }
    if !response.status().is_success() {
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, 64 * 1024)
            .await
            .unwrap_or_default();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        if value["error"].is_object() {
            value["error"]["request_id"] = budget.request_id.clone().into();
            response = Response::from_parts(parts, Body::from(value.to_string()));
        } else {
            response = Response::from_parts(parts, Body::from(bytes));
        }
    }
    response
        .headers_mut()
        .insert("X-Request-ID", budget.request_id.parse().unwrap());
    if !response.headers().get("content-type").is_some_and(|v| {
        v.to_str()
            .unwrap_or_default()
            .starts_with("text/event-stream")
    }) {
        return response;
    }
    let (parts, body) = response.into_parts();
    let mut data = body.into_data_stream();
    // A single buffered frame bounds backpressure; the producer owns all work and its absolute timer.
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let task = tokio::spawn({
        let budget = budget.clone();
        let timed_out = timed_out.clone();
        async move {
            let _run = run;
            loop {
                if budget.expired() {
                    timed_out.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
                let forward = async {
                    match data.next().await {
                        Some(frame) => sender.send(frame).await.map(|_| true).unwrap_or(false),
                        None => false,
                    }
                };
                match tokio::time::timeout_at(budget.deadline, forward).await {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(_) => {
                        timed_out.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                }
            }
            // Dropping the body releases the CallGuard, Adapter stream/process, lane and active task.
        }
    });
    let task = StreamTask(task);
    let stream = async_stream::stream! {
        let _task = task;
        while let Some(frame) = receiver.recv().await {
            if budget.expired() { yield Ok(timeout_frame(&budget, responses)); return; }
            yield frame;
        }
        if timed_out.load(std::sync::atomic::Ordering::Relaxed) { yield Ok(timeout_frame(&budget, responses)); }
    };
    Response::from_parts(parts, Body::from_stream(stream))
}
struct StreamTask(tokio::task::JoinHandle<()>);
impl Drop for StreamTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
fn timeout_frame(budget: &ExecutionBudget, responses: bool) -> axum::body::Bytes {
    let failure = serde_json::json!({"error":{"message":"Request deadline exceeded","type":"localmux_error","code":"LMX_PROCESS_TIMEOUT","error_code":"LMX_PROCESS_TIMEOUT","reason_code":"overall_deadline","limiting_layer":"gateway","retryable":false,"request_id":budget.request_id}});
    let frame = if responses {
        let value = serde_json::json!({"type":"response.failed","sequence_number":budget.next_sequence(),"response":{"id":budget.response_id.lock().unwrap().clone(),"object":"response","status":"failed","output":[],"error":failure["error"]}});
        format!("event: response.failed\ndata: {}\n\n", value)
    } else {
        format!("data: {}\n\ndata: [DONE]\n\n", failure)
    };
    frame.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use localmux_adapter_sdk::{
        Adapter, AdapterError, NormalizedRequest, NormalizedResponse, NormalizedStream,
    };
    use localmux_domain::{AdapterKind, HealthStatus, ModelDescriptor};
    use std::time::Duration;
    struct Agent;
    #[async_trait]
    impl Adapter for Agent {
        fn id(&self) -> &'static str {
            "synthetic"
        }
        fn kind(&self) -> AdapterKind {
            AdapterKind::Agent
        }
        async fn health(&self) -> HealthStatus {
            HealthStatus::Healthy
        }
        async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
            Ok(vec![])
        }
        async fn invoke(&self, _: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
            Err(AdapterError::ProcessFailed)
        }
        async fn invoke_stream(
            &self,
            _: NormalizedRequest,
        ) -> Result<NormalizedStream, AdapterError> {
            Err(AdapterError::ProcessFailed)
        }
    }
    #[tokio::test]
    async fn queue_interval_and_execution_share_the_admission_deadline() {
        let state = super::super::GatewayState::new(localmux_storage::open_memory().await.unwrap());
        tokio::time::pause();
        let initial = ExecutionBudget::default();
        let permit = state.acquire_agent(&Agent, &initial).await.unwrap();
        let budget = ExecutionBudget::default();
        let pending = tokio::spawn({
            let state = state.clone();
            let budget = budget.clone();
            async move { state.acquire_agent(&Agent, &budget).await }
        });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(25)).await;
        drop(permit);
        let permit = pending.await.unwrap().unwrap();
        let timer = budget.measure(Phase::Adapter);
        tokio::time::sleep(Duration::from_secs(10)).await;
        drop(timer);
        let measured = budget.snapshot();
        assert_eq!(measured.queue_wait_ms, Some(25_000));
        assert!(measured.interval_wait_ms.unwrap() <= 1);
        assert!((10_000..=10_005).contains(&measured.adapter_ms.unwrap()));
        assert_eq!(measured.model_execution_ms, None);
        assert_eq!(
            measured.upstream_cancel_verified.as_deref(),
            Some("UNKNOWN")
        );
        assert!((35_000..=35_005).contains(&budget.accepted.elapsed().as_millis()));
        assert!(budget.deadline - tokio::time::Instant::now() <= Duration::from_secs(565));
        drop(permit);
        drop(
            state
                .acquire_agent(&Agent, &ExecutionBudget::default())
                .await
                .unwrap(),
        );
        let expired_interval = ExecutionBudget {
            deadline: tokio::time::Instant::now() + Duration::from_secs(1),
            ..Default::default()
        };
        let error = state
            .acquire_agent(&Agent, &expired_interval)
            .await
            .unwrap_err();
        assert_eq!(error.status(), axum::http::StatusCode::GATEWAY_TIMEOUT);
        tokio::time::advance(Duration::from_secs(5)).await;
        assert!(state
            .acquire_agent(&Agent, &ExecutionBudget::default())
            .await
            .is_ok());
    }
    #[tokio::test]
    async fn queue_deadline_precedes_start_and_does_not_leak_a_lane() {
        let state = super::super::GatewayState::new(localmux_storage::open_memory().await.unwrap());
        tokio::time::pause();
        let permit = state
            .acquire_agent(&Agent, &ExecutionBudget::default())
            .await
            .unwrap();
        let error = state
            .acquire_agent(&Agent, &ExecutionBudget::default())
            .await
            .unwrap_err();
        assert_eq!(error.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(error.headers()["Retry-After"], "5");
        let budget = ExecutionBudget {
            deadline: tokio::time::Instant::now() + Duration::from_secs(2),
            ..Default::default()
        };
        let error = state.acquire_agent(&Agent, &budget).await.unwrap_err();
        assert_eq!(error.status(), axum::http::StatusCode::GATEWAY_TIMEOUT);
        drop(permit);
        assert!(state
            .acquire_agent(&Agent, &ExecutionBudget::default())
            .await
            .is_ok());
    }
    #[tokio::test]
    async fn an_unconsumed_sse_body_expires_and_releases_the_lane_without_being_polled() {
        use axum::{routing::post, Router};
        use http_body_util::BodyExt;
        use tower::ServiceExt;
        let state = super::super::GatewayState::new(localmux_storage::open_memory().await.unwrap());
        let app = Router::new().route("/v1/chat/completions", post(|axum::extract::State(state): axum::extract::State<super::super::GatewayState>, axum::extract::Extension(budget): axum::extract::Extension<ExecutionBudget>| async move {
            let permit = state.acquire_agent(&Agent, &budget).await.unwrap();
            let body = async_stream::stream! {
                let _permit = permit;
                yield Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"data: premature-success\n\n"));
                std::future::pending::<()>().await;
            };
            Response::builder().header("content-type", "text/event-stream").body(Body::from_stream(body)).unwrap()
        })).layer(axum::middleware::from_fn_with_state(state.clone(), admit)).with_state(state.clone());
        tokio::time::pause();
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        tokio::task::yield_now().await;
        assert_eq!(state.active_tasks(), 1);
        tokio::time::advance(Duration::from_secs(601)).await;
        tokio::task::yield_now().await;
        assert_eq!(state.active_tasks(), 0);
        let lane = state
            .agent_lanes
            .lock()
            .await
            .get("synthetic")
            .unwrap()
            .clone();
        assert!(lane.try_lock().is_ok());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("LMX_PROCESS_TIMEOUT"));
        assert!(!text.contains("premature-success"));
    }
}
