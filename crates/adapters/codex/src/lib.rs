use async_trait::async_trait;
use localmux_adapter_sdk::{
    collect_events, text_stream, Adapter, AdapterError, EventStream, FinishReason,
    NormalizedRequest, NormalizedResponse, NormalizedStream, ResponseMetadata, SessionEventStream,
    SessionStream, StreamEvent, TokenUsage,
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
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout, Command},
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
            localmux_adapter_sdk::cli::raw_output(&self.executable, &["--version"]),
        )
        .await
        .map_err(|_| AdapterError::TimeoutAt {
            layer: localmux_adapter_sdk::TimeoutLayer::CliCommand,
            limit: Duration::from_secs(5),
        })??;
        parse_version(&String::from_utf8_lossy(&output)).ok_or(AdapterError::Unsupported)
    }
    pub async fn connect(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        let version = self.version().await?;
        let protocol = self.public_protocol().await;
        let mut process = AppServer::start(&self.executable).await?;
        let (account, _) = process
            .request("account/read", json!({"refreshToken":false}))
            .await?;
        if account["account"].is_null() && account["requiresOpenaiAuth"] == true {
            return Err(AdapterError::AuthRequired);
        }
        process.models(&version, protocol).await
    }
    async fn public_protocol(&self) -> PublicProtocol {
        // The installed CLI owns its schema. Refresh alongside each catalog read,
        // including after in-place upgrades; no version allowlist or persisted fallback.
        let Ok(dir) = tempfile::tempdir() else {
            return PublicProtocol::default();
        };
        let Some(out) = dir.path().to_str() else {
            return PublicProtocol::default();
        };
        let result = localmux_adapter_sdk::cli::command_output(
            Command::new(&self.executable),
            &["app-server", "generate-json-schema", "--out", out],
            Duration::from_secs(10),
        )
        .await;
        if !result.is_ok_and(|output| output.status.success()) {
            return PublicProtocol::default();
        }
        let start_path = dir.path().join("v2/ThreadStartParams.json");
        let resume_path = dir.path().join("v2/ThreadResumeParams.json");
        let turn_path = dir.path().join("v2/TurnStartParams.json");
        let (start, resume, turn) = tokio::join!(
            read_public_schema(&start_path),
            read_public_schema(&resume_path),
            read_public_schema(&turn_path),
        );
        PublicProtocol {
            session_resume: instruction_field(&resume, "threadId"),
            system: instruction_field(&start, "baseInstructions")
                && instruction_field(&resume, "baseInstructions"),
            developer: instruction_field(&start, "developerInstructions")
                && instruction_field(&resume, "developerInstructions"),
            effort: turn
                .as_ref()
                .filter(|schema| schema["type"] == "object")
                .and_then(|schema| {
                    public_strings(&schema["properties"]["effort"], schema, 0, 1, true)
                }),
            observed: true,
        }
    }
    async fn prepare_turn(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<(AppServer, String, String, Vec<Value>, Option<String>), AdapterError> {
        let instructions = request.text_instructions(true)?;
        let version = self.version().await?;
        let protocol = self.public_protocol().await;
        let model = request
            .model
            .strip_prefix("codex/")
            .ok_or(AdapterError::ModelUnavailable)?;
        if model.is_empty() {
            return Err(AdapterError::ModelUnavailable);
        }
        let mut process = AppServer::start(&self.executable).await?;
        let chat = process
            .models(&version, protocol)
            .await?
            .into_iter()
            .find(|m| m.id == request.model)
            .ok_or(AdapterError::ModelUnavailable)?
            .chat;
        request.parameters.validate(&chat)?;
        if (instructions.system.is_some() && chat.system != localmux_domain::Support::Supported)
            || (instructions.developer.is_some()
                && chat.developer != localmux_domain::Support::Supported)
        {
            return Err(AdapterError::UnsupportedMessages);
        }
        let cwd = process.cwd().to_string_lossy().into_owned();
        // Only public config layers are inspected; retain server names, never values or credentials.
        let (configuration, _) = process
            .request("config/read", json!({"includeLayers":true}))
            .await?;
        let mut disabled_mcp = serde_json::Map::new();
        if let Some(layers) = configuration["layers"].as_array() {
            for layer in layers {
                if let Some(servers) = layer["config"]["mcp_servers"].as_object() {
                    for name in servers.keys() {
                        disabled_mcp.insert(name.clone(), json!({"enabled":false}));
                    }
                }
            }
        }
        let config = json!({"mcp_servers":disabled_mcp,"web_search":"disabled","project_doc_max_bytes":0,
            "features":{"shell_tool":false,"unified_exec":false,"apps":false,"plugins":false,"multi_agent":false,"browser_use":false,"computer_use":false,"code_mode":false}});
        let method = if session.is_some() {
            "thread/resume"
        } else {
            "thread/start"
        };
        let mut params = match session {
            Some(id) => json!({"threadId":id,"approvalPolicy":"never","sandbox":"read-only"}),
            None => {
                json!({"model":model,"cwd":cwd,"approvalPolicy":"never","sandbox":"read-only","serviceName":"localmux"})
            }
        };
        params["config"] = config;
        // Explicit client instructions replace the corresponding default instruction layers.
        if chat.system == localmux_domain::Support::Supported {
            params["baseInstructions"] = json!(instructions.system.unwrap_or_default());
        }
        if chat.developer == localmux_domain::Support::Supported {
            params["developerInstructions"] = json!(instructions.developer.unwrap_or_default());
        }
        let (thread, _) = process.request(method, params).await?;
        let actual_model = thread["model"].as_str().map(str::to_owned);
        let id = thread["thread"]["id"]
            .as_str()
            .ok_or(AdapterError::ProcessFailed)?
            .to_owned();
        let mut turn_params = json!({"threadId":id,"input":[{"type":"text","text":request.prompt}],"model":model,"cwd":cwd,"approvalPolicy":"never","sandboxPolicy":{"type":"readOnly"}});
        if let Some(effort) = request.parameters.reasoning_effort {
            turn_params["effort"] = json!(effort);
        }
        let (turn, pending) = process.request("turn/start", turn_params).await?;
        let turn_id = turn["turn"]["id"]
            .as_str()
            .ok_or(AdapterError::ProcessFailed)?
            .to_owned();
        Ok((process, id, turn_id, pending, actual_model))
    }
    pub async fn invoke_with_session(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<NormalizedResponse, AdapterError> {
        let (events, id) = self.stream_with_session(request, session).await?;
        let mut response = collect_events(events).await?;
        response.provider_session_id = Some(id);
        Ok(response)
    }
    async fn stream_with_session(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<(EventStream, String), AdapterError> {
        let normalization =
            (request.messages.len() > 1).then(|| "leading_instructions_v1".to_owned());
        let cli_idle_timeout = request.cli_idle_timeout;
        let (mut process, id, turn_id, pending, actual_model) =
            self.prepare_turn(request, session).await?;
        let session_id = id.clone();
        Ok((
            Box::pin(async_stream::try_stream! {
                let mut pending = pending.into_iter();
                let mut saw_delta = false;
                let mut final_text = String::new();
                let mut metadata = ResponseMetadata { actual_model, actual_model_source: Some("app_server_thread".into()), normalization, ..Default::default() };
                loop {
                    let event = match pending.next() { Some(event) => event, None => process.read(cli_idle_timeout, localmux_adapter_sdk::TimeoutLayer::CliIdle).await? };
                    if event["params"]["threadId"].as_str().is_some_and(|value| value != id) { continue; }
                    if event["params"]["turnId"].as_str().is_some_and(|value| value != turn_id) { continue; }
                    match event["method"].as_str() {
                        Some("error") => Err(map_turn_error(&event["params"]["error"]))?,
                        Some("item/agentMessage/delta") => if let Some(delta) = event["params"]["delta"].as_str() {
                            saw_delta = true; yield StreamEvent::Delta(delta.to_owned());
                        },
                        Some("item/completed") if event["params"]["item"]["type"] == "agentMessage" => {
                            if let Some(text) = event["params"]["item"]["text"].as_str() { final_text = text.to_owned(); }
                        },
                        Some("thread/tokenUsage/updated") => {
                            if let Some(usage) = codex_usage(&event["params"]["tokenUsage"]["last"]) {
                                metadata.usage = Some(usage.clone()); yield StreamEvent::Usage(usage);
                            }
                        },
                        Some("model/rerouted") => { metadata.actual_model = event["params"]["toModel"].as_str().map(str::to_owned); metadata.actual_model_source = Some("app_server_rerouted".into()); },
                        Some("item/started") if event["params"]["item"]["type"].as_str().is_some_and(|kind| !matches!(kind, "agentMessage" | "userMessage" | "reasoning")) => Err(AdapterError::Unsupported)?,
                        Some("turn/completed") => {
                            if event["params"]["turn"]["status"] == "interrupted" { metadata.finish_reason = FinishReason::Cancelled; metadata.finish_source = Some("provider_reported".into()); yield StreamEvent::Completed(metadata); break; }
                            if event["params"]["turn"]["status"] != "completed" { Err(map_turn_error(&event["params"]["turn"]["error"]))?; }
                            if !saw_delta && !final_text.is_empty() { yield StreamEvent::Delta(final_text); }
                            metadata.finish_reason = FinishReason::from_public(event["params"]["turn"]["finishReason"].as_str());
                            metadata.finish_source = (metadata.finish_reason != FinishReason::Unknown).then(|| "provider_reported".into());
                            yield StreamEvent::Completed(metadata);
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
            .map(|(stream, _)| text_stream(stream))
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
            .map(|(stream, _)| text_stream(stream))
    }
    async fn invoke_events(&self, request: NormalizedRequest) -> Result<EventStream, AdapterError> {
        self.stream_with_session(request, None)
            .await
            .map(|(stream, _)| stream)
    }
    async fn invoke_session_events(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<SessionEventStream, AdapterError> {
        let (stream, id) = self.stream_with_session(request, session).await?;
        Ok(SessionEventStream {
            stream,
            provider_session_id: Arc::new(Mutex::new(Some(id))),
        })
    }
    async fn invoke_session_stream(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<SessionStream, AdapterError> {
        let (stream, id) = self.stream_with_session(request, session).await?;
        Ok(SessionStream {
            stream: text_stream(stream),
            provider_session_id: Arc::new(Mutex::new(Some(id))),
        })
    }
}

struct AppServer {
    child: localmux_adapter_sdk::process::OwnedChild,
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
        let mut command = Command::new(executable);
        configure_system_proxy(&mut command).await;
        command
            .arg("app-server")
            .current_dir(dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = localmux_adapter_sdk::process::OwnedChild::spawn(command)
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
        process.request("initialize", json!({"clientInfo":{"name":"localmux","title":"LocalAiMux","version":env!("CARGO_PKG_VERSION")},"capabilities":{"explicitGatewayOauth":true}})).await?;
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
    async fn read(
        &mut self,
        wait: Duration,
        layer: localmux_adapter_sdk::TimeoutLayer,
    ) -> Result<Value, AdapterError> {
        timeout(
            wait,
            localmux_adapter_sdk::cli::read_json_line(&mut self.stdout),
        )
        .await
        .map_err(|_| AdapterError::TimeoutAt { layer, limit: wait })??
        .ok_or(AdapterError::ProcessFailed)
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
        let mut pending_bytes = 0usize;
        loop {
            let message = self
                .read(
                    Duration::from_secs(15),
                    localmux_adapter_sdk::TimeoutLayer::CliRpc,
                )
                .await?;
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
            pending_bytes += message.to_string().len();
            pending.push(message);
            if pending.len() > 1000 || pending_bytes > 4 * 1024 * 1024 {
                return Err(AdapterError::ProcessFailed);
            }
        }
    }
    async fn models(
        &mut self,
        version: &str,
        protocol: PublicProtocol,
    ) -> Result<Vec<ModelDescriptor>, AdapterError> {
        let (result, _) = self
            .request("model/list", json!({"limit":100,"includeHidden":false}))
            .await?;
        let mut models = parse_models(&result)?;
        for model in &mut models {
            model.chat.source =
                "当前安装 CLI 的公开 Schema、型号元数据与 Adapter 映射；真实效果未验证".into();
            model.chat.system = if protocol.system {
                localmux_domain::Support::Supported
            } else {
                localmux_domain::Support::Unknown
            };
            model.chat.developer = if protocol.developer {
                localmux_domain::Support::Supported
            } else {
                localmux_domain::Support::Unknown
            };
            match &protocol.effort {
                None => model.chat.reasoning_efforts.clear(),
                Some(values) if !values.is_empty() => model
                    .chat
                    .reasoning_efforts
                    .retain(|effort| values.contains(effort)),
                Some(_) => {}
            }
            if model
                .chat
                .default_reasoning_effort
                .as_ref()
                .is_some_and(|effort| !model.chat.reasoning_efforts.contains(effort))
            {
                model.chat.default_reasoning_effort = None;
            }
            model.chat.describe(Some(version.into()), "app_server_v2");
            model
                .chat
                .describe_session_resume(if protocol.session_resume {
                    localmux_domain::Support::Supported
                } else {
                    localmux_domain::Support::Unknown
                });
            for name in ["system", "developer", "reasoning_effort", "session_resume"] {
                if let Some(detail) = model.chat.details.get_mut(name) {
                    for evidence in &mut detail.evidence {
                        evidence.source_type = "installed_public_schema_and_adapter_mapping".into();
                    }
                    if detail.effective_support == localmux_domain::Support::Unknown {
                        detail.unknown_reason = Some(
                            if protocol.observed {
                                "public_schema_field_unverified"
                            } else {
                                "public_schema_unavailable"
                            }
                            .into(),
                        );
                    }
                }
            }
            model
                .chat
                .public_candidate("json_schema", "guarantee_unverified");
            if let Some(detail) = model.chat.details.get_mut("image_input") {
                detail.native_support = if model
                    .chat
                    .native_input_modalities
                    .iter()
                    .any(|v| v == "image")
                {
                    localmux_domain::Support::Supported
                } else {
                    localmux_domain::Support::Unknown
                };
                detail.public_interface_support = detail.native_support;
            }
            if let Some(detail) = model.chat.details.get_mut("reasoning_effort") {
                detail.native_support = detail.effective_support;
            }
        }
        Ok(models)
    }
}
impl Drop for AppServer {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

// GUI launches do not inherit shell proxy variables. Use only the user's existing
// fixed macOS HTTP(S) proxy, scoped to this child; explicit environment wins.
async fn configure_system_proxy(_command: &mut Command) {
    #[cfg(target_os = "macos")]
    {
        if [
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
        ]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
        {
            return;
        }
        let Ok(Ok(output)) = timeout(
            Duration::from_secs(2),
            Command::new("/usr/sbin/scutil")
                .arg("--proxy")
                .kill_on_drop(true)
                .output(),
        )
        .await
        else {
            return;
        };
        if !output.status.success() {
            return;
        }
        let snapshot = String::from_utf8_lossy(&output.stdout);
        for (kind, env_name) in [("HTTPS", "HTTPS_PROXY"), ("HTTP", "HTTP_PROXY")] {
            if let Some(proxy) = system_http_proxy(&snapshot, kind) {
                _command.env(env_name, proxy);
            }
        }
        let no_proxy = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .unwrap_or_default();
        let bypass = format!("{no_proxy},localhost,127.0.0.1,::1");
        _command.env("NO_PROXY", &bypass).env("no_proxy", bypass);
    }
}
#[cfg(any(target_os = "macos", test))]
fn system_http_proxy(snapshot: &str, kind: &str) -> Option<String> {
    let field = |name: &str| {
        snapshot.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == name).then(|| value.trim())
        })
    };
    if field(&format!("{kind}Enable"))? != "1" {
        return None;
    }
    let host = field(&format!("{kind}Proxy"))?;
    let port: u16 = field(&format!("{kind}Port"))?.parse().ok()?;
    if host.is_empty()
        || port == 0
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
    {
        return None;
    }
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    Some(format!("http://{host}:{port}"))
}
fn map_turn_error(error: &Value) -> AdapterError {
    let info = &error["codexErrorInfo"];
    match info.as_str() {
        Some("unauthorized") => return AdapterError::AuthRequired,
        Some("contextWindowExceeded") => return AdapterError::ContextLimit,
        Some("badRequest") => {
            return localmux_adapter_sdk::parameter_error(
                "request",
                "upstream_validation",
                "public_interface",
            )
        }
        Some("cyberPolicy" | "misalignmentPolicyViolation") => return AdapterError::Refusal,
        Some("usageLimitExceeded" | "sessionBudgetExceeded") => {
            return AdapterError::QuotaExhausted
        }
        Some("rateLimitExceeded") => return AdapterError::RateLimited,
        Some("serverOverloaded" | "flexUnavailable") => return AdapterError::ProviderUnavailable,
        _ => {}
    }
    for kind in [
        "httpConnectionFailed",
        "responseStreamConnectionFailed",
        "responseStreamDisconnected",
        "responseTooManyFailedAttempts",
    ] {
        if let Some(detail) = info.get(kind) {
            return match detail["httpStatusCode"].as_u64() {
                Some(401 | 403) => AdapterError::AuthRequired,
                Some(429) => AdapterError::RateLimited,
                _ => AdapterError::ProviderUnavailable,
            };
        }
    }
    map_rpc_error(error)
}

#[derive(Default)]
struct PublicProtocol {
    session_resume: bool,
    system: bool,
    developer: bool,
    effort: Option<Vec<String>>,
    observed: bool,
}

async fn read_public_schema(path: &Path) -> Option<Value> {
    const MAX_SCHEMA_BYTES: u64 = 1024 * 1024;
    let metadata = tokio::fs::symlink_metadata(path).await.ok()?;
    if !metadata.is_file() || metadata.len() > MAX_SCHEMA_BYTES {
        return None;
    }
    let file = tokio::fs::File::open(path).await.ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_SCHEMA_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .ok()?;
    if bytes.len() as u64 > MAX_SCHEMA_BYTES {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn instruction_field(schema: &Option<Value>, name: &str) -> bool {
    let Some(schema) = schema else { return false };
    schema["type"] == "object"
        && public_strings(&schema["properties"][name], schema, 0, 0, false).is_some()
}

fn public_strings(
    field: &Value,
    root: &Value,
    depth: usize,
    minimum: u64,
    allow_enum: bool,
) -> Option<Vec<String>> {
    // Only local definitions and known string/effort-enum contracts are mapped. Unknown
    // schemas/constraints stay unknown instead of approximating a JSON Schema validator.
    let string_type = field["type"] == "string"
        || field["type"]
            .as_array()
            .is_some_and(|types| types.iter().any(|t| t == "string"));
    if depth > 8
        || (field.get("type").is_some() && !string_type)
        || (string_type && field.get("anyOf").is_some())
        || [
            "const",
            "pattern",
            "maxLength",
            "allOf",
            "oneOf",
            "not",
            "if",
            "format",
        ]
        .iter()
        .any(|key| field.get(*key).is_some())
        || (field.get("enum").is_some() && (!allow_enum || !string_type))
        || field
            .get("minLength")
            .is_some_and(|length| length.as_u64().is_none_or(|length| length > minimum))
    {
        return None;
    }
    if let Some(reference) = field["$ref"].as_str() {
        return reference
            .strip_prefix('#')
            .and_then(|pointer| root.pointer(pointer))
            .and_then(|target| public_strings(target, root, depth + 1, minimum, allow_enum));
    }
    if string_type {
        return match field.get("enum") {
            None => Some(Vec::new()),
            Some(values) if allow_enum => {
                let values = values.as_array()?;
                if values.is_empty() {
                    return None;
                }
                values
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .filter(|value| !value.is_empty())
                            .map(str::to_owned)
                    })
                    .collect()
            }
            Some(_) => None,
        };
    }
    field["anyOf"].as_array().and_then(|types| {
        types
            .iter()
            .find_map(|part| public_strings(part, root, depth + 1, minimum, allow_enum))
    })
}

fn codex_capabilities(item: &Value) -> localmux_domain::ChatCapabilities {
    let mut chat = localmux_domain::ChatCapabilities::single_turn();
    chat.native_input_modalities = item["inputModalities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    chat.reasoning_efforts = item["supportedReasoningEfforts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["reasoningEffort"].as_str())
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect();
    chat.default_reasoning_effort = item["defaultReasoningEffort"].as_str().map(str::to_owned);
    chat
}
fn codex_usage(last: &Value) -> Option<TokenUsage> {
    // `last` is the turn snapshot; `total` is cumulative session usage and is never added.
    Some(TokenUsage {
        input_tokens: Some(last["inputTokens"].as_u64()?),
        output_tokens: Some(last["outputTokens"].as_u64()?),
        total_tokens: Some(last["totalTokens"].as_u64()?),
        reasoning_tokens: last["reasoningOutputTokens"].as_u64(),
        cached_input_tokens: last["cachedInputTokens"].as_u64(),
        source: "provider_reported".into(),
        scope: "last_turn".into(),
    })
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
                chat: codex_capabilities(item),
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
    } else if matches!(error["code"].as_i64(), Some(-32600 | -32602)) {
        localmux_adapter_sdk::parameter_error(
            "request",
            "protocol_invalid_request",
            "public_interface",
        )
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
    fn public_schema_constraints_and_reference_cycles_stay_unknown() {
        let root = json!({"definitions":{"cycle":{"$ref":"#/definitions/cycle"}}});
        for field in [
            json!({"type":"string","maxLength":10}),
            json!({"type":"string","pattern":"^fixed$"}),
            json!({"type":"string","minLength":2}),
            json!({"anyOf":[{"type":"string"}],"enum":["low"]}),
            json!({"type":"number","$ref":"#/definitions/text"}),
            json!({"type":"string","anyOf":[{"enum":["low"]}]}),
            json!({"$ref":"https://example.invalid/schema"}),
            json!({"$ref":"#/definitions/cycle"}),
        ] {
            assert!(public_strings(&field, &root, 0, 1, true).is_none());
        }
        let effort = json!({"type":"string","minLength":1,"enum":["low","high"]});
        assert_eq!(
            public_strings(&effort, &root, 0, 1, true),
            Some(vec!["low".into(), "high".into()])
        );
        assert!(public_strings(&effort, &root, 0, 0, false).is_none());
    }
    #[test]
    fn fixed_system_proxy_is_validated_and_disabled_proxy_is_ignored() {
        let snapshot = "HTTPSEnable : 1\nHTTPSProxy : 127.0.0.1\nHTTPSPort : 10808";
        assert_eq!(
            system_http_proxy(snapshot, "HTTPS").as_deref(),
            Some("http://127.0.0.1:10808")
        );
        assert!(
            system_http_proxy(&snapshot.replace("Enable : 1", "Enable : 0"), "HTTPS").is_none()
        );
        assert!(system_http_proxy(&snapshot.replace("10808", "0"), "HTTPS").is_none());
        assert!(
            system_http_proxy(&snapshot.replace("127.0.0.1", "user:secret@host"), "HTTPS")
                .is_none()
        );
        assert_eq!(
            system_http_proxy(&snapshot.replace("127.0.0.1", "::1"), "HTTPS").as_deref(),
            Some("http://[::1]:10808")
        );
    }
    #[test]
    fn structured_turn_errors_preserve_auth_quota_and_network_categories() {
        for (info, expected) in [
            (json!("unauthorized"), "auth"),
            (json!("usageLimitExceeded"), "quota"),
            (json!("rateLimitExceeded"), "rate"),
            (
                json!({"responseStreamDisconnected":{"httpStatusCode":null}}),
                "network",
            ),
            (
                json!({"httpConnectionFailed":{"httpStatusCode":401}}),
                "auth",
            ),
        ] {
            let result =
                map_turn_error(&json!({"message":"private detail", "codexErrorInfo":info}));
            assert!(matches!(
                (expected, result),
                ("auth", AdapterError::AuthRequired)
                    | ("quota", AdapterError::QuotaExhausted)
                    | ("rate", AdapterError::RateLimited)
                    | ("network", AdapterError::ProviderUnavailable)
            ));
        }
    }
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
        assert!(matches!(
            map_rpc_error(&json!({"code":-32602,"message":"invalid synthetic model argument"})),
            AdapterError::InvalidParameter { .. }
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
