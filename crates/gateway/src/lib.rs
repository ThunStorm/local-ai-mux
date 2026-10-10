mod execution;
mod recording;
mod responses;

use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use futures_util::StreamExt;
use localmux_adapter_sdk::{
    Adapter, AdapterError, MockEcho, NormalizedMessage, NormalizedRequest, ResponseMetadata,
    StreamEvent, TokenUsage,
};
use localmux_api_types::{
    AssistantMessage, ChatRequest, ChatResponse, Choice, ModelList, ModelObject, Usage,
};
use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
use localmux_quota::{QuotaRegistry, QuotaSnapshot};
use localmux_router::{CircuitBreaker, RouteCandidate, RouteError, RoutePolicy};
use localmux_security::{KeyRateLimiter, KeyRestrictions};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    convert::Infallible,
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};
use uuid::Uuid;

pub const DEFAULT_PORT: u16 = 8317;
pub const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(180);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
const AGENT_QUEUE_TIMEOUT: Duration = Duration::from_secs(30);
const AGENT_REQUEST_INTERVAL: Duration = Duration::from_secs(5);
const MAX_BODY: usize = 1024 * 1024;
type CatalogCache = Arc<Mutex<HashMap<String, (Instant, i64, Vec<ModelDescriptor>)>>>;
type AgentLane = Arc<Mutex<Option<tokio::time::Instant>>>;
type AgentPermit = OwnedMutexGuard<Option<tokio::time::Instant>>;

#[derive(Clone)]
pub struct GatewayState {
    pub pool: SqlitePool,
    adapters: Arc<RwLock<Vec<Arc<dyn Adapter>>>>,
    policy: Arc<RwLock<RoutePolicy>>,
    breaker: Arc<CircuitBreaker>,
    quotas: Arc<QuotaRegistry>,
    catalog: CatalogCache,
    limiter: Arc<KeyRateLimiter>,
    active_runs: Arc<AtomicUsize>,
    agent_lanes: Arc<Mutex<HashMap<String, AgentLane>>>,
    adapter_runs: Arc<std::sync::Mutex<HashMap<String, usize>>>,
}
struct AdapterRun(Arc<std::sync::Mutex<HashMap<String, usize>>>, String);
impl Drop for AdapterRun {
    fn drop(&mut self) {
        let mut runs = self.0.lock().unwrap();
        if let Some(n) = runs.get_mut(&self.1) {
            *n -= 1;
            if *n == 0 {
                runs.remove(&self.1);
            }
        }
    }
}
struct ActiveRun(Arc<AtomicUsize>);
impl Drop for ActiveRun {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
impl GatewayState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            adapters: Arc::new(RwLock::new(Vec::new())),
            policy: Arc::new(RwLock::new(RoutePolicy::default())),
            breaker: Arc::new(CircuitBreaker::default()),
            quotas: Arc::new(QuotaRegistry::new(Duration::from_secs(60))),
            catalog: Arc::new(Mutex::new(HashMap::new())),
            limiter: Arc::new(KeyRateLimiter::default()),
            active_runs: Arc::new(AtomicUsize::new(0)),
            agent_lanes: Arc::new(Mutex::new(HashMap::new())),
            adapter_runs: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
    pub fn mock(pool: SqlitePool) -> Self {
        let state = Self::new(pool);
        state
            .adapters
            .try_write()
            .expect("new gateway adapters lock")
            .push(Arc::new(MockEcho));
        state
    }
    pub fn active_tasks(&self) -> usize {
        self.active_runs.load(Ordering::Relaxed)
    }
    pub fn warning_count(&self) -> usize {
        self.breaker.open_count(Instant::now())
    }
    fn begin_adapter(&self, id: &str) -> AdapterRun {
        *self
            .adapter_runs
            .lock()
            .unwrap()
            .entry(id.into())
            .or_default() += 1;
        AdapterRun(self.adapter_runs.clone(), id.into())
    }
    /// Local activity/health only; never probes a model or reads private account state.
    pub async fn connection_runtime_status(&self, id: &str) -> serde_json::Value {
        let adapter = self
            .adapters
            .read()
            .await
            .iter()
            .find(|a| a.id() == id)
            .cloned();
        let status = if let Some(adapter) = adapter {
            if !matches!(
                adapter.health().await,
                HealthStatus::Healthy | HealthStatus::Degraded
            ) {
                "UNAVAILABLE"
            } else if !self.breaker.available(id, Instant::now()) {
                "COOLDOWN"
            } else {
                let lane = self.agent_lanes.lock().await.get(id).cloned();
                if lane.is_some_and(|lane| lane.try_lock().is_err()) {
                    "BUSY"
                } else if self
                    .adapter_runs
                    .lock()
                    .unwrap()
                    .get(id)
                    .is_some_and(|n| *n > 0)
                {
                    "ACTIVE"
                } else {
                    "IDLE"
                }
            }
        } else {
            "UNAVAILABLE"
        };
        serde_json::json!({"state":status,"source":"local_gateway_activity_and_adapter_health","observed_at":now()})
    }
    fn begin_run(&self) -> ActiveRun {
        self.active_runs.fetch_add(1, Ordering::Relaxed);
        ActiveRun(self.active_runs.clone())
    }
    async fn acquire_agent(
        &self,
        adapter: &dyn Adapter,
        budget: &execution::ExecutionBudget,
    ) -> Result<Option<AgentPermit>, Box<Response>> {
        if adapter.kind() != AdapterKind::Agent {
            return Ok(None);
        }
        // Current adapters expose one account per adapter; instance IDs must replace this key when multi-account support ships.
        let lane = self
            .agent_lanes
            .lock()
            .await
            .entry(adapter.id().to_owned())
            .or_default()
            .clone();
        let queue_timer = budget.measure(execution::Phase::Queue);
        let mut permit = tokio::time::timeout_at(
            budget.child_deadline(AGENT_QUEUE_TIMEOUT),
            lane.lock_owned(),
        )
        .await
        .map_err(|_| {
            if budget.expired() {
                return Box::new(budget.timeout_response());
            }
            budget.metrics.lock().unwrap().reason_code = Some("agent_queue_timeout".into());
            let mut response = error(
                StatusCode::TOO_MANY_REQUESTS,
                "LMX_RATE_LIMITED",
                "Local CLI connection is busy; retry later",
            );
            response
                .headers_mut()
                .insert("Retry-After", "5".parse().unwrap());
            Box::new(response)
        })?;
        drop(queue_timer);
        let interval_timer = budget.measure(execution::Phase::Interval);
        if let Some(last) = *permit {
            tokio::time::timeout_at(
                budget.deadline,
                tokio::time::sleep_until(last + AGENT_REQUEST_INTERVAL),
            )
            .await
            .map_err(|_| Box::new(budget.timeout_response()))?;
        }
        drop(interval_timer);
        if budget.expired() {
            return Err(Box::new(budget.timeout_response()));
        }
        if let Some(response) = connection_cooldown(self, adapter.id()) {
            return Err(Box::new(response));
        }
        *permit = Some(tokio::time::Instant::now());
        Ok(Some(permit))
    }
    pub async fn invalidate_catalog(&self, id: &str) {
        self.catalog.lock().await.remove(id);
    }
    pub async fn seed_catalog(&self, id: &str, models: Vec<ModelDescriptor>) {
        self.catalog
            .lock()
            .await
            .insert(id.to_owned(), (Instant::now(), now(), models));
    }
    pub async fn invalidate_all_catalog(&self) {
        self.catalog
            .lock()
            .await
            .retain(|_, (time, _, _)| time.elapsed() < Duration::from_secs(30));
    }
    async fn available_candidates(&self) -> Vec<RouteCandidate> {
        let adapters = self.adapters.read().await.clone();
        let mut candidates = Vec::new();
        for adapter in adapters {
            let health = adapter.health().await;
            if !matches!(health, HealthStatus::Healthy | HealthStatus::Degraded) {
                continue;
            }
            // ponytail: one catalog lock serializes provider refreshes; use per-connection locks if measured latency requires it.
            let mut cache = self.catalog.lock().await;
            let mut models = if let Some((_, _, models)) = cache
                .get(adapter.id())
                .filter(|(time, _, _)| time.elapsed() < Duration::from_secs(300))
            {
                models.clone()
            } else {
                match adapter.list_models().await {
                    Ok(models) => {
                        cache.insert(adapter.id().into(), (Instant::now(), now(), models.clone()));
                        models
                    }
                    Err(_) => {
                        cache.remove(adapter.id());
                        continue;
                    }
                }
            };
            for model in &mut models {
                model.chat.expire_evidence(now());
            }
            if let Some((_, _, cached)) = cache.get_mut(adapter.id()) {
                *cached = models.clone();
            }
            drop(cache);
            for model in models {
                candidates.push(RouteCandidate {
                    model_id: model.id,
                    connection_id: adapter.id().into(),
                    cost_mode: model.cost_mode,
                    quota_multiplier: None,
                    reserve_available: self
                        .quotas
                        .get(adapter.id())
                        .is_some_and(|snapshot| snapshot.fresh_confirmed_available(now(), 600)),
                    health: health.clone(),
                });
            }
        }
        candidates
    }
    pub async fn route_status(&self) -> Vec<(String, Option<String>)> {
        let candidates = self.available_candidates().await;
        let policy = self.route_policy().await;
        ["free", "cheap", "balanced", "best"]
            .into_iter()
            .map(|alias| {
                let target = localmux_router::select(
                    alias,
                    &candidates,
                    &policy,
                    &self.breaker,
                    Instant::now(),
                )
                .ok()
                .map(|candidate| candidate.model_id.clone());
                (alias.to_owned(), target)
            })
            .collect()
    }
    pub async fn route_policy(&self) -> RoutePolicy {
        self.policy.read().await.clone()
    }
    pub async fn set_route_policy(&self, policy: RoutePolicy) {
        *self.policy.write().await = policy;
    }
    pub fn quota_snapshot(&self, id: &str) -> Option<QuotaSnapshot> {
        self.quotas.get(id)
    }
    pub fn restore_quota_snapshot(&self, snapshot: QuotaSnapshot) {
        self.quotas.update(snapshot);
    }
    pub async fn refresh_quota_for(&self, id: &str, force: bool) {
        if !force && !self.quotas.should_refresh(id, Instant::now()) {
            return;
        }
        let adapter = self
            .adapters
            .read()
            .await
            .iter()
            .find(|adapter| adapter.id() == id)
            .cloned();
        if let Some(adapter) = adapter {
            if let Ok(buckets) = adapter.get_quotas().await {
                if !buckets.is_empty()
                    && buckets
                        .iter()
                        .all(|item| item.source == localmux_quota::QuotaSource::ProviderReported)
                    && localmux_storage::save_quota_buckets(&self.pool, id, &buckets)
                        .await
                        .is_ok()
                {
                    let snapshot = buckets[0].as_snapshot();
                    let _ = localmux_storage::save_quota_snapshot(&self.pool, &snapshot).await;
                    self.quotas.update(snapshot);
                }
            }
        }
    }
    pub async fn refresh_quotas(&self, force: bool) {
        let adapters = self.adapters.read().await.clone();
        for adapter in adapters {
            self.refresh_quota_for(adapter.id(), force).await;
        }
    }
    pub async fn register(&self, adapter: Arc<dyn Adapter>) {
        self.invalidate_catalog(adapter.id()).await;
        let mut adapters = self.adapters.write().await;
        adapters.retain(|current| current.id() != adapter.id());
        adapters.push(adapter);
    }
    pub async fn unregister(&self, id: &str) {
        self.invalidate_catalog(id).await;
        self.adapters
            .write()
            .await
            .retain(|adapter| adapter.id() != id);
    }
    pub async fn create_key(&self, scopes: &[&str]) -> anyhow::Result<(String, String)> {
        self.create_key_with_policy(scopes, KeyRestrictions::default())
            .await
    }
    pub async fn create_key_with_policy(
        &self,
        scopes: &[&str],
        policy: KeyRestrictions,
    ) -> anyhow::Result<(String, String)> {
        if !policy.valid() {
            anyhow::bail!("invalid API key policy");
        }
        let id = Uuid::new_v4().to_string();
        let key = format!("lmx_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let names: Vec<String> = scopes.iter().map(|scope| (*scope).to_owned()).collect();
        localmux_storage::insert_api_key_with_policy(
            &self.pool,
            &id,
            &hash_key(&key),
            &names,
            &policy,
        )
        .await?;
        Ok((id, key))
    }
    pub async fn key_matches_id(&self, id: &str, raw: &str) -> anyhow::Result<bool> {
        Ok(localmux_storage::active_api_key(&self.pool, &hash_key(raw))
            .await?
            .is_some_and(|record| record.id == id))
    }
    pub async fn verify_reported_model(&self, model: &str) -> Result<(), AdapterError> {
        let budget = execution::ExecutionBudget::default();
        let _run = self.begin_run();
        let mut guard = recording::CallGuard::start(
            &self.pool,
            None,
            "model_test",
            "model_test",
            model,
            false,
            true,
        )
        .await
        .map_err(|_| AdapterError::ProcessFailed)?
        .with_budget(budget.clone());
        let call = guard.call();
        let result = tokio::time::timeout_at(
            budget.deadline,
            self.verify_reported_model_inner(model, call.clone()),
        )
        .await
        .unwrap_or(Err(AdapterError::Timeout));
        if let Err(kind) = &result {
            call.fail(adapter_error_details(kind).1);
        }
        guard
            .persist()
            .await
            .map_err(|_| AdapterError::ProcessFailed)?;
        result
    }
    async fn verify_reported_model_inner(
        &self,
        model: &str,
        call: recording::Call,
    ) -> Result<(), AdapterError> {
        let candidate = self
            .available_candidates()
            .await
            .into_iter()
            .find(|candidate| candidate.model_id == model)
            .ok_or(AdapterError::ModelUnavailable)?;
        let adapter = self
            .adapters
            .read()
            .await
            .iter()
            .find(|adapter| adapter.id() == candidate.connection_id)
            .cloned()
            .ok_or(AdapterError::ModelUnavailable)?;
        call.target(
            model,
            adapter.id(),
            &format!("test_{}", Uuid::new_v4().simple()),
            None,
        );
        let _permit = self
            .acquire_agent(adapter.as_ref(), &call.budget())
            .await
            .map_err(|_| AdapterError::RateLimited)?;
        let prompt = "Reply with OK.";
        call.request(&serde_json::json!([{"role":"user","content":prompt}]))
            .await
            .map_err(|_| AdapterError::ProcessFailed)?;
        call.budget().metrics.lock().unwrap().auth_route_ms = Some(call.budget().elapsed_ms());
        let _adapter_run = self.begin_adapter(adapter.id());
        let adapter_timer = call.budget().measure(execution::Phase::Adapter);
        let response = tokio::time::timeout_at(
            call.budget().deadline,
            adapter.invoke(NormalizedRequest {
                model: model.into(),
                prompt: prompt.into(),
                messages: vec![NormalizedMessage {
                    role: "user".into(),
                    content: prompt.into(),
                }],
                parameters: Default::default(),
            }),
        )
        .await
        .map_err(|_| AdapterError::Timeout)??;
        drop(adapter_timer);
        call.answer(&response.text)
            .await
            .map_err(|_| AdapterError::ProcessFailed)?;
        if response.text.trim().is_empty() {
            return Err(AdapterError::ProcessFailed);
        }
        call.result(&response.metadata);
        Ok(())
    }
}
fn hash_key(key: &str) -> String {
    format!("{:x}", Sha256::digest(key.as_bytes()))
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn router(state: GatewayState) -> Router {
    Router::new()
        .route(
            "/api/health",
            get(|State(state): State<GatewayState>| async move {
                Json(serde_json::json!({"status":"ok","active_tasks":state.active_tasks(),"execution_limits":{"request_timeout_seconds":REQUEST_TIMEOUT.as_secs(),"stream_idle_timeout_seconds":STREAM_IDLE_TIMEOUT.as_secs(),"agent_max_concurrency":1,"agent_queue_timeout_seconds":AGENT_QUEUE_TIMEOUT.as_secs(),"agent_request_interval_seconds":AGENT_REQUEST_INTERVAL.as_secs()}}))
            }),
        )
        .route("/v1/models", get(models))
        .route("/api/chat/models", get(chat_models))
        .route("/v1/chat/completions", post(chat))
        .route("/v1/responses", post(responses::create))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY))
        .layer(axum::middleware::from_fn_with_state(state.clone(), execution::admit))
        .with_state(state)
}
pub async fn serve(state: GatewayState, port: u16) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
    serve_on(listener, state).await
}
pub async fn serve_on(
    listener: tokio::net::TcpListener,
    state: GatewayState,
) -> std::io::Result<()> {
    axum::serve(listener, router(state)).await
}

async fn authorized(
    state: &GatewayState,
    headers: &HeaderMap,
    scope: &str,
) -> Result<localmux_storage::ApiKeyRecord, Box<Response>> {
    let Some(key) = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    else {
        return Err(Box::new(error(
            StatusCode::UNAUTHORIZED,
            "LMX_AUTH_REQUIRED",
            "API key required",
        )));
    };
    if !key.starts_with("lmx_") {
        return Err(Box::new(error(
            StatusCode::UNAUTHORIZED,
            "LMX_AUTH_REQUIRED",
            "Invalid API key",
        )));
    }
    match localmux_storage::active_api_key(&state.pool, &hash_key(key)).await {
        Ok(Some(record))
            if record.scopes.iter().any(|item| item == scope) && record.policy.valid() =>
        {
            if state.limiter.allow(
                &record.id,
                record.policy.rate_limit_per_minute,
                Instant::now(),
            ) {
                Ok(record)
            } else {
                Err(Box::new(error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "LMX_RATE_LIMITED",
                    "Local API key rate limit exceeded",
                )))
            }
        }
        Ok(_) => Err(Box::new(error(
            StatusCode::FORBIDDEN,
            "LMX_PERMISSION_DENIED",
            "Key is invalid, revoked, or lacks scope",
        ))),
        Err(_) => Err(Box::new(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "LMX_INTERNAL_ERROR",
            "Credential lookup failed",
        ))),
    }
}
fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(serde_json::json!({"error":{"message":message,"type":"localmux_error","code":code,"error_code":code,"param":null,"limiting_layer":"unknown","retryable":false,"request_id":Uuid::new_v4().to_string()}}))).into_response()
}
fn target_allowed(
    record: &localmux_storage::ApiKeyRecord,
    requested: &str,
    actual: &str,
    adapter: &dyn Adapter,
) -> bool {
    record
        .policy
        .permits(requested, actual, adapter.id(), adapter.kind())
        && (adapter.kind() != AdapterKind::Agent
            || record.scopes.iter().any(|scope| scope == "agent:invoke"))
}
async fn models(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    let key = match authorized(&state, &headers, "models:read").await {
        Ok(key) => key,
        Err(response) => return *response,
    };
    let candidates = state.available_candidates().await;
    let adapters = state.adapters.read().await.clone();
    let permitted = |candidate: &RouteCandidate, requested: &str| {
        adapters
            .iter()
            .find(|adapter| adapter.id() == candidate.connection_id)
            .is_some_and(|adapter| {
                target_allowed(&key, requested, &candidate.model_id, adapter.as_ref())
            })
    };
    let mut data: Vec<ModelObject> = candidates
        .iter()
        .filter(|candidate| permitted(candidate, &candidate.model_id))
        .map(|candidate| ModelObject {
            id: candidate.model_id.clone(),
            object: "model",
            created: 0,
            owned_by: candidate.connection_id.clone(),
        })
        .collect();
    let policy = state.route_policy().await;
    for alias in ["free", "cheap", "balanced", "best"] {
        if localmux_router::select(alias, &candidates, &policy, &state.breaker, Instant::now())
            .is_ok_and(|candidate| permitted(candidate, alias))
        {
            data.push(ModelObject {
                id: alias.into(),
                object: "model",
                created: 0,
                owned_by: "localmux".into(),
            });
        }
    }
    Json(ModelList {
        object: "list",
        data,
    })
    .into_response()
}
fn route_error(kind: RouteError) -> Response {
    match kind {
        RouteError::FreePoolExhausted => error(
            StatusCode::TOO_MANY_REQUESTS,
            "LMX_FREE_POOL_EXHAUSTED",
            "No confirmed free model is available",
        ),
        RouteError::ModelUnavailable | RouteError::NoEligibleRoute => error(
            StatusCode::NOT_FOUND,
            "LMX_MODEL_UNAVAILABLE",
            "No eligible model route",
        ),
    }
}

async fn chat_models(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    let key = match authorized(&state, &headers, "models:read").await {
        Ok(key) => key,
        Err(response) => return *response,
    };
    let candidates = state.available_candidates().await;
    let adapters = state.adapters.read().await.clone();
    let catalog = state.catalog.lock().await;
    let mut data = Vec::new();
    for candidate in candidates {
        let Some(adapter) = adapters
            .iter()
            .find(|adapter| adapter.id() == candidate.connection_id)
        else {
            continue;
        };
        if !target_allowed(
            &key,
            &candidate.model_id,
            &candidate.model_id,
            adapter.as_ref(),
        ) {
            continue;
        }
        if let Some(model) = catalog
            .values()
            .flat_map(|(_, _, models)| models)
            .find(|model| model.id == candidate.model_id)
        {
            let age = catalog
                .get(adapter.id())
                .map(|(at, _, _)| at.elapsed().as_secs())
                .unwrap_or(300);
            let observed_at = catalog.get(adapter.id()).map(|(_, at, _)| *at).unwrap_or(0);
            let runtime = state.connection_runtime_status(adapter.id()).await;
            data.push(serde_json::json!({"runtime":runtime,"catalog":{"observed_at":observed_at,"expires_at":observed_at+300,"ttl_seconds":300,"status":if age < 300 { "CURRENT" } else { "STALE" },"source":"adapter_public_catalog","live_verified":false},"id":model.id,"provider":model.provider_id,"stream":model.supports_streaming,"chat":model.chat,"connection_id":model.connection_id.as_deref().unwrap_or(adapter.id()),"adapter":adapter.id(),"identity_scope":"adapter_compat"}));
        }
    }
    Json(serde_json::json!({"schema_version":2,"data":data})).into_response()
}
fn breaker_error(state: &GatewayState, adapter_id: &str, kind: &AdapterError) {
    if matches!(
        kind,
        AdapterError::RateLimited | AdapterError::QuotaExhausted
    ) {
        let state = state.clone();
        let id = adapter_id.to_owned();
        tokio::spawn(async move {
            state.invalidate_catalog(&id).await;
            state.refresh_quota_for(&id, false).await;
        });
    }
    let duration = match kind {
        AdapterError::RateLimited => Some(Duration::from_secs(60)),
        AdapterError::QuotaExhausted => Some(Duration::from_secs(300)),
        AdapterError::ProviderUnavailable | AdapterError::ProcessFailed | AdapterError::Timeout => {
            Some(Duration::from_secs(30))
        }
        _ => None,
    };
    if let Some(duration) = duration {
        state.breaker.open(adapter_id, Instant::now(), duration);
    }
}
async fn chat(
    State(state): State<GatewayState>,
    Extension(budget): Extension<execution::ExecutionBudget>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let key = match authorized(&state, &headers, "chat:invoke").await {
        Ok(key) => key,
        Err(response) => return *response,
    };
    let request: ChatRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "LMX_INVALID_REQUEST",
                "Malformed chat request",
            )
        }
    };
    let source = if headers
        .get("x-localmux-source")
        .is_some_and(|v| v == "playground")
    {
        "playground"
    } else {
        "api"
    };
    let guard = match recording::CallGuard::start(
        &state.pool,
        Some(&key.id),
        source,
        "chat",
        &request.model,
        request.stream,
        request.store.unwrap_or(true),
    )
    .await
    {
        Ok(guard) => guard.with_budget(budget),
        Err(_) => return recording::persistence_error(),
    };
    let call = guard.call();
    call.budget().metrics.lock().unwrap().request_bytes = Some(body.len() as u64);
    if call
        .request(&serde_json::to_value(&request.messages).unwrap_or_default())
        .await
        .is_err()
    {
        call.fail("LMX_INTERNAL_ERROR");
        return guard.response(recording::persistence_error()).await;
    }
    let response = chat_inner(state, key, request, call).await;
    guard.response(response).await
}
async fn chat_inner(
    state: GatewayState,
    key: localmux_storage::ApiKeyRecord,
    request: ChatRequest,
    call: recording::Call,
) -> Response {
    if request.stream_options.is_some() && !request.stream {
        return protocol_error(
            "LMX_INVALID_REQUEST",
            "stream_options",
            "stream_required",
            "gateway",
        );
    }
    if request.model.is_empty()
        || request.messages.is_empty()
        || request
            .temperature
            .is_some_and(|v| !v.is_finite() || !(0.0..=2.0).contains(&v))
    {
        return error(
            StatusCode::BAD_REQUEST,
            "LMX_INVALID_REQUEST",
            "Invalid model, messages, or temperature",
        );
    }
    if request.max_tokens == Some(0)
        || request
            .top_p
            .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
    {
        return error(
            StatusCode::BAD_REQUEST,
            "LMX_INVALID_REQUEST",
            "max_tokens must be positive and top_p must be between 0 and 1",
        );
    }
    if request.messages.iter().any(|message| {
        !matches!(
            message.role.as_str(),
            "system" | "developer" | "user" | "assistant"
        ) || !message.content.is_string()
    }) {
        return error(
            StatusCode::BAD_REQUEST,
            "LMX_INVALID_REQUEST",
            "Only text system, user, and assistant messages are supported",
        );
    }
    let mut body_started = false;
    for message in &request.messages {
        if matches!(message.role.as_str(), "system" | "developer") {
            if body_started {
                return protocol_error(
                    "LMX_UNSUPPORTED_MESSAGES",
                    "messages",
                    "non_leading_instruction",
                    "gateway",
                );
            }
        } else {
            body_started = true;
        }
    }
    let Some(prompt) = request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .and_then(|message| message.content.as_str())
    else {
        return error(
            StatusCode::BAD_REQUEST,
            "LMX_INVALID_REQUEST",
            "A user message is required",
        );
    };
    let adapters = state.adapters.read().await.clone();
    let requested = request.model.as_str();
    let virtual_route = matches!(requested, "free" | "cheap" | "balanced" | "best");
    let actual_model = if virtual_route {
        let candidates = state.available_candidates().await;
        let policy = state.route_policy().await;
        match localmux_router::select(
            requested,
            &candidates,
            &policy,
            &state.breaker,
            Instant::now(),
        ) {
            Ok(candidate) => candidate.model_id.clone(),
            Err(kind) => return route_error(kind),
        }
    } else {
        request.model.clone()
    };
    let Some(adapter) = adapters
        .iter()
        .find(|adapter| actual_model.starts_with(&format!("{}/", adapter.id())))
    else {
        return error(
            StatusCode::NOT_FOUND,
            "LMX_MODEL_UNAVAILABLE",
            "Model unavailable",
        );
    };
    if !target_allowed(&key, &request.model, &actual_model, adapter.as_ref()) {
        return error(
            StatusCode::FORBIDDEN,
            "LMX_PERMISSION_DENIED",
            "API key cannot invoke this route",
        );
    }
    let adapter_id = adapter.id().to_owned();
    call.target(&actual_model, &adapter_id, "", None);
    let parameters = localmux_adapter_sdk::ChatParameters {
        temperature: request.temperature,
        top_p: request.top_p,
        max_tokens: request.max_tokens,
        max_completion_tokens: request.max_completion_tokens,
        reasoning_effort: request.reasoning_effort.clone(),
        response_format: match request
            .response_format
            .clone()
            .map(serde_json::from_value)
            .transpose()
        {
            Ok(value) => value,
            Err(_) => {
                return protocol_error(
                    "LMX_INVALID_REQUEST",
                    "response_format",
                    "malformed_format",
                    "gateway",
                )
            }
        },
    };
    state.available_candidates().await;
    let descriptor = state
        .catalog
        .lock()
        .await
        .get(&adapter_id)
        .and_then(|(_, _, models)| models.iter().find(|model| model.id == actual_model))
        .cloned();
    let Some(descriptor) = descriptor else {
        return adapter_error(AdapterError::ModelUnavailable);
    };
    if let Err(kind) = parameters.validate(&descriptor.chat) {
        return adapter_error(kind);
    }
    if request.stream && !descriptor.supports_streaming {
        return protocol_error(
            "LMX_UNSUPPORTED_PARAMETER",
            "stream",
            "unsupported",
            "adapter",
        );
    }
    if request.messages.iter().any(|m| m.role == "system")
        && descriptor.chat.system != localmux_domain::Support::Supported
    {
        return protocol_error(
            "LMX_UNSUPPORTED_MESSAGES",
            "messages",
            "not_implemented",
            "adapter",
        );
    }
    if request.messages.iter().any(|m| m.role == "developer")
        && descriptor.chat.developer != localmux_domain::Support::Supported
    {
        return protocol_error(
            "LMX_UNSUPPORTED_MESSAGES",
            "messages",
            "role_priority_unmapped",
            descriptor
                .chat
                .details
                .get("developer")
                .and_then(|d| d.limiting_layer.as_deref())
                .unwrap_or("adapter"),
        );
    }
    if !virtual_route {
        if let Some(response) = connection_cooldown(&state, &adapter_id) {
            return response;
        }
        let exact = [RouteCandidate {
            model_id: actual_model.clone(),
            connection_id: adapter_id.clone(),
            cost_mode: CostMode::Unknown,
            quota_multiplier: None,
            reserve_available: false,
            health: adapter.health().await,
        }];
        if let Err(kind) = localmux_router::select(
            &actual_model,
            &exact,
            &state.route_policy().await,
            &state.breaker,
            Instant::now(),
        ) {
            return route_error(kind);
        }
    }
    let normalized = NormalizedRequest {
        model: actual_model.clone(),
        prompt: prompt.to_owned(),
        messages: request
            .messages
            .iter()
            .map(|message| NormalizedMessage {
                role: message.role.clone(),
                content: message.content.as_str().unwrap_or_default().to_owned(),
            })
            .collect(),
        parameters,
    };
    if descriptor.chat.history != localmux_domain::Support::Supported {
        if let Err(kind) = normalized
            .text_instructions(descriptor.chat.developer == localmux_domain::Support::Supported)
        {
            return adapter_error(kind);
        }
    }
    let normalization = (normalized.messages.len() > 1
        && descriptor.chat.history != localmux_domain::Support::Supported)
        .then(|| "leading_instructions_v1".to_owned());
    let include_usage = request
        .stream_options
        .as_ref()
        .is_some_and(|options| options.include_usage);
    call.normalization(normalization.as_deref());
    {
        let budget = call.budget();
        let mut metrics = budget.metrics.lock().unwrap();
        metrics.auth_route_ms = Some(budget.elapsed_ms());
        metrics.route_kind = Some(
            if virtual_route {
                "virtual_route"
            } else {
                "explicit_model"
            }
            .into(),
        );
    }
    let permit = match state.acquire_agent(adapter.as_ref(), &call.budget()).await {
        Ok(permit) => permit,
        Err(response) => return *response,
    };
    let deadline = call.budget().deadline;
    let id = format!("chatcmpl-{}", Uuid::new_v4().simple());
    let created = now();
    call.target(&actual_model, &adapter_id, &id, None);
    let adapter_run = state.begin_adapter(&adapter_id);
    let adapter_timer = call.budget().measure(execution::Phase::Adapter);
    if request.stream {
        let startup_timer = call.budget().measure(execution::Phase::Startup);
        let mut provider_stream = match tokio::time::timeout_at(
            call.budget().child_deadline(Duration::from_secs(30)),
            adapter.invoke_events(normalized),
        )
        .await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(kind)) => {
                breaker_error(&state, &adapter_id, &kind);
                return adapter_error(kind);
            }
            Err(_) => {
                state
                    .breaker
                    .open(&adapter_id, Instant::now(), Duration::from_secs(30));
                return error(
                    StatusCode::GATEWAY_TIMEOUT,
                    "LMX_PROCESS_TIMEOUT",
                    "Upstream first response timed out",
                );
            }
        };
        drop(startup_timer);
        let model = actual_model.clone();
        let route_state = state.clone();
        let events = async_stream::stream! {
            let _permit = permit;
            let role = serde_json::json!({"id":id,"object":"chat.completion.chunk","created":created,"model":model,"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]});
            yield Ok::<Event, Infallible>(Event::default().data(role.to_string()));
            let mut failure = None;
            let mut metadata = ResponseMetadata::default();
            let mut completed = false;
            let mut output_bytes = 0usize;
            loop {
                let next_deadline = (tokio::time::Instant::now() + STREAM_IDLE_TIMEOUT).min(deadline);
                match tokio::time::timeout_at(next_deadline, provider_stream.next()).await {
                    Ok(Some(Ok(StreamEvent::Delta(delta)))) if !completed => {
                        call.first_output(&delta);
                        output_bytes += delta.len();
                        if output_bytes > 8 * 1024 * 1024 { failure = Some("LMX_OUTPUT_TOO_LARGE"); break; }
                        if call.answer(&delta).await.is_err() { failure = Some("LMX_INTERNAL_ERROR"); break; }
                        let chunk = serde_json::json!({"id":id,"object":"chat.completion.chunk","created":created,"model":model,"choices":[{"index":0,"delta":{"content":delta},"finish_reason":null}]});
                        yield Ok(Event::default().data(chunk.to_string()));
                    },
                    Ok(Some(Ok(StreamEvent::Usage(usage)))) if !completed => { metadata.usage = usage.validated(); call.metadata(&metadata); },
                    Ok(Some(Ok(StreamEvent::Completed(mut reported)))) if !completed => {
                        reported = reported.validated(); reported.usage = reported.usage.or(metadata.usage.take()); metadata = reported; call.metadata(&metadata); completed = true;
                    },
                    Ok(Some(Ok(_))) => { failure = Some("LMX_INVALID_RESPONSE"); break; },
                    Ok(None) => { if completed { route_state.breaker.success(&adapter_id); } else { failure = Some("LMX_STREAM_INTERRUPTED"); } break; },
                    Ok(Some(Err(kind))) => { breaker_error(&route_state, &adapter_id, &kind); failure = Some(adapter_error_details(&kind).1); break; },
                    Err(_) => { route_state.breaker.open(&adapter_id, Instant::now(), Duration::from_secs(30)); failure = Some("LMX_PROCESS_TIMEOUT"); break; },
                }
            }
            drop(adapter_timer);
            drop(adapter_run);
            if let Some(code) = failure {
                call.fail(code);
                yield Ok(Event::default().data(serde_json::json!({"error":{"message":"Upstream stream failed or timed out","type":"localmux_error","code":code,"error_code":code,"param":null,"limiting_layer":"public_interface","retryable":false,"request_id":call.budget().request_id}}).to_string()));
            } else {
                call.result(&metadata);
                let last = serde_json::json!({"id":id,"object":"chat.completion.chunk","created":created,"model":model,"choices":[{"index":0,"delta":{},"finish_reason":metadata.finish_reason.openai()}],"localmux":wire_metadata(&metadata, &id, &model, normalization.as_deref(), &call.budget())});
                yield Ok(Event::default().data(last.to_string()));
                if include_usage {
                    let usage = serde_json::json!({"id":id,"object":"chat.completion.chunk","created":created,"model":model,"choices":[],"usage":wire_usage(metadata.usage.as_ref()),"localmux":wire_metadata(&metadata,&id,&model,normalization.as_deref(), &call.budget())});
                    yield Ok(Event::default().data(usage.to_string()));
                }
            }
            yield Ok(Event::default().data("[DONE]"));
        };
        return Sse::new(events)
            .keep_alive(KeepAlive::default())
            .into_response();
    }
    let mut response = match tokio::time::timeout_at(deadline, adapter.invoke(normalized)).await {
        Ok(Ok(response)) => {
            state.breaker.success(&adapter_id);
            response
        }
        Ok(Err(error_kind)) => {
            breaker_error(&state, &adapter_id, &error_kind);
            return adapter_error(error_kind);
        }
        Err(_) => {
            return error(
                StatusCode::GATEWAY_TIMEOUT,
                "LMX_PROCESS_TIMEOUT",
                "Upstream timed out",
            )
        }
    };
    response.metadata = response.metadata.validated();
    drop(adapter_timer);
    drop(adapter_run);
    if call.answer(&response.text).await.is_err() {
        return recording::persistence_error();
    }
    call.result(&response.metadata);
    Json(ChatResponse {
        localmux: wire_metadata(
            &response.metadata,
            &id,
            &actual_model,
            normalization.as_deref(),
            &call.budget(),
        ),
        id,
        object: "chat.completion",
        created,
        model: actual_model,
        choices: vec![Choice {
            index: 0,
            message: AssistantMessage {
                role: "assistant",
                content: response.text,
            },
            finish_reason: response.metadata.finish_reason.openai(),
        }],
        usage: wire_usage(response.metadata.usage.as_ref()),
    })
    .into_response()
}
fn protocol_error(code: &str, param: &str, reason: &str, layer: &str) -> Response {
    let param = match param {
        "messages"
        | "temperature"
        | "top_p"
        | "max_tokens"
        | "max_completion_tokens"
        | "reasoning_effort"
        | "response_format"
        | "stream"
        | "stream_options" => param,
        _ => "parameter",
    };
    let reason = match reason {
        "conflict"
        | "out_of_range"
        | "not_implemented"
        | "unknown_constraints"
        | "guarantee_unverified"
        | "non_leading_instruction"
        | "role_priority_unmapped"
        | "role_or_history_unmapped"
        | "unsupported"
        | "stream_required"
        | "malformed_format"
        | "evidence_expired_or_undated" => reason,
        _ => "unverified_adapter_diagnostic",
    };
    let layer = match layer {
        "gateway" | "adapter" | "public_interface" | "model" | "account" | "unknown" => layer,
        _ => "unknown",
    };
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":{"message":"Request cannot be honored by the selected public interface","type":"localmux_error","code":code,"error_code":code,"param":param,"reason_code":reason,"limiting_layer":layer,"retryable":false,"request_id":Uuid::new_v4().to_string()}}))).into_response()
}
fn wire_usage(usage: Option<&TokenUsage>) -> Option<Usage> {
    usage.map(|usage| Usage {
        prompt_tokens: usage.input_tokens,
        completion_tokens: usage.output_tokens,
        total_tokens: usage.total_tokens,
    })
}
fn wire_metadata(
    metadata: &ResponseMetadata,
    id: &str,
    resolved_model: &str,
    normalization: Option<&str>,
    budget: &execution::ExecutionBudget,
) -> serde_json::Value {
    let metadata = metadata.clone().validated();
    serde_json::json!({
        "request_id":budget.request_id, "response_id":id,
        "resolved_model":localmux_adapter_sdk::public_model_id(resolved_model).then_some(resolved_model),
        "actual_model":metadata.actual_model, "actual_model_source":metadata.actual_model_source,
        "finish_reason":metadata.finish_reason, "finish_source":metadata.finish_source,
        "usage":metadata.usage, "usage_status":if metadata.usage.is_some() { "provider_reported" } else { "unknown" },
        "normalization":normalization.filter(|v| *v == "leading_instructions_v1").or(metadata.normalization.as_deref()),
        "cancellation_guarantee":"local_receive_only", "upstream_cancel_verified":"UNKNOWN",
        "timing":budget.snapshot(),
    })
}
fn connection_cooldown(state: &GatewayState, adapter_id: &str) -> Option<Response> {
    (!state.breaker.available(adapter_id, Instant::now())).then(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "LMX_PROVIDER_COOLDOWN",
            "Provider is temporarily cooling down; retry manually later",
        )
    })
}
fn adapter_error(kind: AdapterError) -> Response {
    if let AdapterError::InvalidParameter {
        param,
        reason,
        layer,
    } = &kind
    {
        return protocol_error("LMX_UNSUPPORTED_PARAMETER", param, reason, layer);
    }
    if matches!(kind, AdapterError::UnsupportedMessages) {
        return protocol_error(
            "LMX_UNSUPPORTED_MESSAGES",
            "messages",
            "role_or_history_unmapped",
            "adapter",
        );
    }
    let (status, code, message) = adapter_error_details(&kind);
    error(status, code, message)
}
fn adapter_error_details(kind: &AdapterError) -> (StatusCode, &'static str, &'static str) {
    match kind {
        AdapterError::Timeout => (
            StatusCode::GATEWAY_TIMEOUT,
            "LMX_PROCESS_TIMEOUT",
            "Upstream idle timed out",
        ),
        AdapterError::ContextLimit => (
            StatusCode::BAD_REQUEST,
            "LMX_CONTEXT_LIMIT",
            "Upstream input context limit exceeded",
        ),
        AdapterError::Refusal => (
            StatusCode::BAD_REQUEST,
            "LMX_REFUSAL",
            "Upstream refused this request",
        ),
        AdapterError::AuthRequired => (
            StatusCode::UNAUTHORIZED,
            "LMX_AUTH_REQUIRED",
            "Provider authentication required",
        ),
        AdapterError::ModelUnavailable => (
            StatusCode::NOT_FOUND,
            "LMX_MODEL_UNAVAILABLE",
            "Model unavailable",
        ),
        AdapterError::QuotaExhausted => (
            StatusCode::TOO_MANY_REQUESTS,
            "LMX_QUOTA_EXHAUSTED",
            "Provider quota exhausted",
        ),
        AdapterError::RateLimited => (
            StatusCode::TOO_MANY_REQUESTS,
            "LMX_RATE_LIMITED",
            "Provider rate limited",
        ),
        AdapterError::Unsupported => (
            StatusCode::NOT_IMPLEMENTED,
            "LMX_ADAPTER_UNSUPPORTED_VERSION",
            "Adapter unsupported",
        ),
        AdapterError::UnsupportedMessages => (
            StatusCode::BAD_REQUEST,
            "LMX_UNSUPPORTED_MESSAGES",
            "This model cannot preserve chat roles or message history",
        ),
        AdapterError::UnsupportedParameter | AdapterError::InvalidParameter { .. } => (
            StatusCode::BAD_REQUEST,
            "LMX_UNSUPPORTED_PARAMETER",
            "Parameter unsupported, unknown, or outside reported range",
        ),
        AdapterError::ProviderUnavailable | AdapterError::ProcessFailed => (
            StatusCode::BAD_GATEWAY,
            "LMX_PROVIDER_UNAVAILABLE",
            "Provider unavailable",
        ),
    }
}
