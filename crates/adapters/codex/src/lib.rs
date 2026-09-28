use async_trait::async_trait;
use localmux_adapter_sdk::{
    Adapter, AdapterError, NormalizedRequest, NormalizedResponse, NormalizedStream, SessionStream,
};
use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
use localmux_quota::{QuotaBucket, QuotaSnapshot, QuotaSource};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
    time::timeout,
};

/// One request owns one app-server process. This bounds cancellation and keeps
/// provider sessions resumable by ID without sharing mutable child state.
pub struct CodexAdapter {
    executable: PathBuf,
}
impl CodexAdapter {
    pub fn new(executable: PathBuf) -> Self {
        Self { executable }
    }
    pub fn executable(&self) -> &Path {
        &self.executable
    }
    pub async fn version(&self) -> Result<String, AdapterError> {
        let output = timeout(
            Duration::from_secs(5),
            Command::new(&self.executable).arg("--version").output(),
        )
        .await
        .map_err(|_| AdapterError::ProcessFailed)?
        .map_err(|_| AdapterError::ProviderUnavailable)?;
        if !output.status.success() {
            return Err(AdapterError::Unsupported);
        }
        parse_version(&String::from_utf8_lossy(&output.stdout)).ok_or(AdapterError::Unsupported)
    }
    pub async fn connect(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        self.version().await?;
        let mut process = AppServer::start(&self.executable).await?;
        let (account, _) = process
            .request("account/read", json!({"refreshToken":false}))
            .await?;
        if account["account"].is_null() && account["requiresOpenaiAuth"] == true {
            return Err(AdapterError::AuthRequired);
        }
        process.models().await
    }
    async fn prepare_turn(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<(AppServer, String, String, Vec<Value>), AdapterError> {
        request.single_user_prompt()?;
        let model = request
            .model
            .strip_prefix("codex/")
            .ok_or(AdapterError::ModelUnavailable)?;
        if model.is_empty() {
            return Err(AdapterError::ModelUnavailable);
        }
        let mut process = AppServer::start(&self.executable).await?;
        let cwd = process.cwd().to_string_lossy().into_owned();
        let method = if session.is_some() {
            "thread/resume"
        } else {
            "thread/start"
        };
        let params = match session {
            Some(id) => json!({"threadId":id,"approvalPolicy":"never","sandbox":"readOnly"}),
            None => {
                json!({"model":model,"cwd":cwd,"approvalPolicy":"never","sandbox":"readOnly","serviceName":"localmux"})
            }
        };
        let (thread, _) = process.request(method, params).await?;
        let id = thread["thread"]["id"]
            .as_str()
            .ok_or(AdapterError::ProcessFailed)?
            .to_owned();
        let (turn, pending) = process.request("turn/start", json!({"threadId":id,"input":[{"type":"text","text":request.prompt}],"model":model,"cwd":cwd,"approvalPolicy":"never","sandboxPolicy":{"type":"readOnly"}})).await?;
        let turn_id = turn["turn"]["id"]
            .as_str()
            .ok_or(AdapterError::ProcessFailed)?
            .to_owned();
        Ok((process, id, turn_id, pending))
    }
    pub async fn invoke_with_session(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<NormalizedResponse, AdapterError> {
        let (mut process, id, turn_id, pending) = self.prepare_turn(request, session).await?;
        process
            .collect_turn(&id, &turn_id, pending)
            .await
            .map(|text| NormalizedResponse {
                text,
                provider_session_id: Some(id),
            })
    }
    async fn stream_with_session(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<(NormalizedStream, String), AdapterError> {
        let (mut process, id, turn_id, pending) = self.prepare_turn(request, session).await?;
        let session_id = id.clone();
        Ok((
            Box::pin(async_stream::try_stream! {
                let mut pending = pending.into_iter();
                let mut saw_delta = false;
                let mut final_text = String::new();
                loop {
                    let event = match pending.next() { Some(event) => event, None => process.read(Duration::from_secs(120)).await? };
                    if event["params"]["threadId"].as_str().is_some_and(|value| value != id) { continue; }
                    if event["params"]["turnId"].as_str().is_some_and(|value| value != turn_id) { continue; }
                    match event["method"].as_str() {
                        Some("item/agentMessage/delta") => if let Some(delta) = event["params"]["delta"].as_str() {
                            saw_delta = true; yield delta.to_owned();
                        },
                        Some("item/completed") if event["params"]["item"]["type"] == "agentMessage" => {
                            if let Some(text) = event["params"]["item"]["text"].as_str() { final_text = text.to_owned(); }
                        },
                        Some("turn/completed") => {
                            if event["params"]["turn"]["status"] != "completed" { Err(AdapterError::ProcessFailed)?; }
                            if !saw_delta && !final_text.is_empty() { yield final_text; }
                            break;
                        },
                        _ => {},
                    }
                }
            }),
            session_id,
        ))
    }
}

#[async_trait]
impl Adapter for CodexAdapter {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Agent
    }
    async fn health(&self) -> HealthStatus {
        if self.executable.is_file() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Offline
        }
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        self.connect().await
    }
    async fn get_quota(&self) -> Result<Option<QuotaSnapshot>, AdapterError> {
        Ok(self
            .get_quotas()
            .await?
            .first()
            .map(QuotaBucket::as_snapshot))
    }
    async fn get_quotas(&self) -> Result<Vec<QuotaBucket>, AdapterError> {
        let mut process = AppServer::start(&self.executable).await?;
        let (result, _) = process
            .request(
                "account/rateLimits/read",
                json!({"excludeResetCreditDetails":true}),
            )
            .await?;
        let observed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AdapterError::ProcessFailed)?
            .as_secs() as i64;
        Ok(parse_quotas(&result, observed_at))
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        self.invoke_with_session(request, None).await
    }
    async fn invoke_stream(
        &self,
        request: NormalizedRequest,
    ) -> Result<NormalizedStream, AdapterError> {
        self.stream_with_session(request, None)
            .await
            .map(|(stream, _)| stream)
    }
    async fn resume(
        &self,
        request: NormalizedRequest,
        provider_session_id: &str,
    ) -> Result<NormalizedResponse, AdapterError> {
        self.invoke_with_session(request, Some(provider_session_id))
            .await
    }
    async fn resume_stream(
        &self,
        request: NormalizedRequest,
        provider_session_id: &str,
    ) -> Result<NormalizedStream, AdapterError> {
        self.stream_with_session(request, Some(provider_session_id))
            .await
            .map(|(stream, _)| stream)
    }
    async fn invoke_session_stream(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<SessionStream, AdapterError> {
        let (stream, id) = self.stream_with_session(request, session).await?;
        Ok(SessionStream {
            stream,
            provider_session_id: Arc::new(Mutex::new(Some(id))),
        })
    }
}

struct AppServer {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    dir: TempDir,
}
impl AppServer {
    async fn start(executable: &Path) -> Result<Self, AdapterError> {
        if !executable.is_file() {
            return Err(AdapterError::ProviderUnavailable);
        }
        let dir = tempfile::tempdir().map_err(|_| AdapterError::ProcessFailed)?;
        let mut child = Command::new(executable)
            .arg("app-server")
            .current_dir(dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| AdapterError::ProviderUnavailable)?;
        let stdin = child.stdin.take().ok_or(AdapterError::ProcessFailed)?;
        let stdout = BufReader::new(child.stdout.take().ok_or(AdapterError::ProcessFailed)?);
        let mut process = Self {
            child,
            stdin,
            stdout,
            next_id: 1,
            dir,
        };
        process.request("initialize", json!({"clientInfo":{"name":"localmux","title":"LocalMux","version":"0.1.0"},"capabilities":{"explicitGatewayOauth":true}})).await?;
        process.send(json!({"method":"initialized"})).await?;
        let (oauth, _) = process
            .request("account/gatewayOAuth/read", json!({}))
            .await?;
        if oauth["required"] == true && oauth["status"] != "succeeded" {
            return Err(AdapterError::AuthRequired);
        }
        Ok(process)
    }
    fn cwd(&self) -> &Path {
        self.dir.path()
    }
    async fn send(&mut self, value: Value) -> Result<(), AdapterError> {
        self.stdin
            .write_all(value.to_string().as_bytes())
            .await
            .map_err(|_| AdapterError::ProcessFailed)?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|_| AdapterError::ProcessFailed)?;
        self.stdin
            .flush()
            .await
            .map_err(|_| AdapterError::ProcessFailed)
    }
    async fn read(&mut self, wait: Duration) -> Result<Value, AdapterError> {
        let mut line = String::new();
        let size = timeout(wait, self.stdout.read_line(&mut line))
            .await
            .map_err(|_| AdapterError::ProcessFailed)?
            .map_err(|_| AdapterError::ProcessFailed)?;
        if size == 0 || size > 1024 * 1024 {
            return Err(AdapterError::ProcessFailed);
        }
        serde_json::from_str(&line).map_err(|_| AdapterError::ProcessFailed)
    }
    async fn request(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<(Value, Vec<Value>), AdapterError> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"id":id,"method":method,"params":params}))
            .await?;
        let mut pending = Vec::new();
        loop {
            let message = self.read(Duration::from_secs(15)).await?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(map_rpc_error(error));
                }
                return Ok((
                    message
                        .get("result")
                        .cloned()
                        .ok_or(AdapterError::ProcessFailed)?,
                    pending,
                ));
            }
            if message.get("id").is_some() && message.get("method").is_some() {
                return Err(AdapterError::Unsupported);
            }
            pending.push(message);
            if pending.len() > 1000 {
                return Err(AdapterError::ProcessFailed);
            }
        }
    }
    async fn models(&mut self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        let (result, _) = self
            .request("model/list", json!({"limit":100,"includeHidden":false}))
            .await?;
        parse_models(&result)
    }
    async fn collect_turn(
        &mut self,
        thread_id: &str,
        turn_id: &str,
        pending: Vec<Value>,
    ) -> Result<String, AdapterError> {
        let mut pending = pending.into_iter();
        let mut final_text = String::new();
        let mut deltas = String::new();
        loop {
            let event = match pending.next() {
                Some(event) => event,
                None => self.read(Duration::from_secs(120)).await?,
            };
            if event["params"]["threadId"]
                .as_str()
                .is_some_and(|id| id != thread_id)
            {
                continue;
            }
            if event["params"]["turnId"]
                .as_str()
                .is_some_and(|id| id != turn_id)
            {
                continue;
            }
            match event["method"].as_str() {
                Some("item/agentMessage/delta") => {
                    if let Some(delta) = event["params"]["delta"].as_str() {
                        deltas.push_str(delta);
                    }
                }
                Some("item/completed") if event["params"]["item"]["type"] == "agentMessage" => {
                    if let Some(text) = event["params"]["item"]["text"].as_str() {
                        final_text = text.to_owned();
                    }
                }
                Some("turn/completed") => {
                    if event["params"]["turn"]["status"] != "completed" {
                        return Err(AdapterError::ProcessFailed);
                    }
                    return Ok(if final_text.is_empty() {
                        deltas
                    } else {
                        final_text
                    });
                }
                _ => {}
            }
        }
    }
}
impl Drop for AppServer {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn parse_models(value: &Value) -> Result<Vec<ModelDescriptor>, AdapterError> {
    let data = value["data"]
        .as_array()
        .ok_or(AdapterError::ProcessFailed)?;
    Ok(data
        .iter()
        .filter_map(|item| {
            let model = item["model"].as_str().or_else(|| item["id"].as_str())?;
            if item["hidden"] == true {
                return None;
            }
            Some(ModelDescriptor {
                id: format!("codex/{model}"),
                provider_id: "codex".into(),
                connection_id: None,
                cost_mode: CostMode::Unknown,
                supports_streaming: true,
                supports_session: true,
            })
        })
        .collect())
}
pub fn parse_version(output: &str) -> Option<String> {
    let mut parts = output.split_whitespace();
    if parts.next()? != "codex-cli" {
        return None;
    }
    let version = parts.next()?;
    if version
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+'))
    {
        Some(version.to_owned())
    } else {
        None
    }
}
fn map_rpc_error(error: &Value) -> AdapterError {
    if error["code"] == -32601 {
        return AdapterError::Unsupported;
    }
    let message = error["message"].as_str().unwrap_or("").to_ascii_lowercase();
    if message.contains("auth") || message.contains("login") || message.contains("sign in") {
        AdapterError::AuthRequired
    } else if message.contains("rate limit") {
        AdapterError::RateLimited
    } else if message.contains("quota") {
        AdapterError::QuotaExhausted
    } else if message.contains("model") {
        AdapterError::ModelUnavailable
    } else {
        AdapterError::ProcessFailed
    }
}

#[cfg(test)]
fn parse_quota(result: &Value, observed_at: i64) -> Option<QuotaSnapshot> {
    parse_quotas(result, observed_at)
        .first()
        .map(QuotaBucket::as_snapshot)
}

fn parse_quotas(result: &Value, observed_at: i64) -> Vec<QuotaBucket> {
    if result["ordinaryUsageAllowed"] != true {
        return Vec::new();
    }
    let bucket = result["rateLimitsByLimitId"]["codex"]
        .as_object()
        .map(|_| &result["rateLimitsByLimitId"]["codex"])
        .unwrap_or(&result["rateLimits"]);
    ["primary", "secondary"]
        .into_iter()
        .filter_map(|id| {
            let window = &bucket[id];
            let used = window["usedPercent"].as_f64()?;
            if !used.is_finite() || !(0.0..=100.0).contains(&used) {
                return None;
            }
            let minutes = window["windowDurationMins"].as_u64();
            Some(QuotaBucket {
                connection_id: "codex".into(),
                bucket_id: id.into(),
                label: minutes.map_or(id.into(), |value| format!("{value} min window")),
                remaining: Some(100.0 - used),
                limit: Some(100.0),
                unit: Some("%".into()),
                reset_at: window["resetsAt"].as_i64(),
                source: QuotaSource::ProviderReported,
                observed_at,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_is_dynamic_and_never_assumes_free() {
        let models = parse_models(
            &serde_json::from_str(include_str!("../tests/fixtures/model_list.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "codex/fixture-model");
        assert_eq!(models[0].cost_mode, CostMode::Unknown);
    }
    #[test]
    fn rpc_auth_error_is_normalized() {
        assert!(matches!(
            map_rpc_error(
                &serde_json::from_str(include_str!("../tests/fixtures/auth_expired.json")).unwrap()
            ),
            AdapterError::AuthRequired
        ));
        assert!(matches!(
            map_rpc_error(
                &serde_json::from_str(include_str!("../tests/fixtures/rate_limit.json")).unwrap()
            ),
            AdapterError::RateLimited
        ));
        assert_eq!(
            parse_version(include_str!("../tests/fixtures/version.txt")).as_deref(),
            Some("0.155.0-alpha.16.4")
        );
    }
    #[test]
    fn official_rate_limit_window_is_a_percentage_not_credit_balance() {
        let value = json!({"ordinaryUsageAllowed":true,"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":25.0,"windowDurationMins":300,"resetsAt":1700000000},"secondary":{"usedPercent":60.0,"windowDurationMins":10080,"resetsAt":1700600000}}}});
        let quota = parse_quota(&value, 1699999000).unwrap();
        assert_eq!(quota.remaining, Some(75.0));
        assert_eq!(quota.limit, Some(100.0));
        assert_eq!(quota.reset_at, Some(1700000000));
        let buckets = parse_quotas(&value, 1699999000);
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].remaining, Some(75.0));
        assert_eq!(buckets[1].remaining, Some(40.0));
        assert_eq!(buckets[1].reset_at, Some(1700600000));
        assert!(parse_quota(&json!({"rateLimits":{"primary":{"usedPercent":null}}}), 0).is_none());
        assert!(parse_quota(
            &json!({"ordinaryUsageAllowed":false,"rateLimits":{"primary":{"usedPercent":25}}}),
            0
        )
        .is_none());
    }
}
