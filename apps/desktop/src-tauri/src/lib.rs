use localmux_adapter_sdk::Adapter;
mod native_material;
mod playground;
use localmux_router::RoutePolicy;
use localmux_secrets::{MemorySecretStore, SecretStore};
use secrecy::SecretString;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock,
    },
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Mutex;

struct GatewayControl {
    state: localmux_gateway::GatewayState,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    mimocode: Mutex<Option<MimoCodeSession>>,
    default_key_lock: Mutex<()>,
    close_to_tray: AtomicBool,
    playground: playground::Control,
}

struct MimoCodeSession {
    executable: String,
    _child: tokio::process::Child,
    _dir: tempfile::TempDir,
    token_id: String,
}

const MIMO_BASE_URL: &str = "https://api.xiaomimimo.com/v1";
const MIMO_SECRET_REF: &str = "provider.mimo";
const MIMO_CONNECTION_REF: &str = "session-credential:mimo";
const DEFAULT_KEY_SETTING: &str = "default_api_key_id";
static SESSION_SECRETS: LazyLock<MemorySecretStore> = LazyLock::new(MemorySecretStore::default);

fn default_key_store() -> &'static MemorySecretStore {
    &SESSION_SECRETS
}
fn default_key_ref(id: &str) -> String {
    format!("api.key.{id}")
}

fn mimo_store() -> &'static MemorySecretStore {
    &SESSION_SECRETS
}

fn mimo_adapter(key: String) -> Result<Arc<dyn Adapter>, String> {
    localmux_adapter_sdk::provider_http::HttpProvider::new(
        "mimo",
        MIMO_BASE_URL,
        SecretString::from(key),
    )
    .map(|adapter| Arc::new(adapter) as Arc<dyn Adapter>)
    .map_err(|error| error.to_string())
}

struct MimoToken {
    id: String,
    key: String,
    base: String,
}

fn trusted_mimocode_base(base: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(base) else {
        return false;
    };
    url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port().is_some()
        && url.path() == "/v1"
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

async fn revoke_mimocode_token(executable: &str, dir: &Path, id: &str) -> Result<(), String> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new(executable)
            .args(["llm-server", "revoke", id])
            .current_dir(dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
    )
    .await
    .map_err(|_| "MiMo Code 令牌撤销超时")?
    .map_err(|_| "MiMo Code 令牌撤销失败")?;
    if output.success() {
        Ok(())
    } else {
        Err("MiMo Code 令牌撤销失败".into())
    }
}

async fn issue_mimocode_token(
    executable: &str,
    dir: &Path,
    models: &[String],
) -> Result<Option<MimoToken>, String> {
    let mut command = tokio::process::Command::new(executable);
    // ponytail: short-lived session token; reconnect after expiry if long-running use is needed.
    command.args([
        "llm-server",
        "issue",
        "--json",
        "--label",
        "LocalAiMux",
        "--ttl",
        "30m",
        "--max-age",
        "30m",
    ]);
    for model in models {
        command.arg("--model").arg(model);
    }
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        command.current_dir(dir).stderr(Stdio::null()).output(),
    )
    .await
    .map_err(|_| "MiMo Code 令牌申请超时")?
    .map_err(|_| "无法启动 MiMo Code")?;
    if !output.status.success() {
        return Err("MiMo Code 令牌申请失败，请在官方 CLI 检查登录状态".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|_| "MiMo Code 令牌格式无效")?;
    let id = value["id"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("MiMo Code 令牌缺少 ID")?;
    let key = value["api_key"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("MiMo Code 令牌缺少 Key")?;
    let Some(base) = value["base_url"].as_str() else {
        revoke_mimocode_token(executable, dir, id).await?;
        return Ok(None);
    };
    if !trusted_mimocode_base(base) {
        revoke_mimocode_token(executable, dir, id).await?;
        return Err("MiMo Code 未提供可信的本机 API 地址".into());
    }
    Ok(Some(MimoToken {
        id: id.into(),
        key: key.into(),
        base: base.into(),
    }))
}

struct AdapterRegistration {
    id: &'static str,
    executable_names: &'static [&'static str],
    create: fn(PathBuf) -> Arc<dyn Adapter>,
}

const ADAPTERS: &[AdapterRegistration] = &[
    AdapterRegistration {
        id: "codex",
        executable_names: &["codex", "codex.exe"],
        create: |path| Arc::new(localmux_adapter_codex::CodexAdapter::new(path)),
    },
    AdapterRegistration {
        id: "qoder",
        executable_names: &["qoder", "qoder.exe"],
        create: |path| Arc::new(localmux_adapter_qoder::QoderAdapter::new(path)),
    },
    AdapterRegistration {
        id: "qoder-cn",
        executable_names: &["qodercn", "qodercn.exe"],
        create: |path| Arc::new(localmux_adapter_qoder::QoderAdapter::new_cn(path)),
    },
    AdapterRegistration {
        id: "codebuddy",
        executable_names: &["codebuddy", "codebuddy.exe", "cbc", "cbc.exe"],
        create: |path| Arc::new(localmux_adapter_codebuddy::CodeBuddyAdapter::new(path)),
    },
];

fn adapter_for(id: &str, path: PathBuf) -> Option<Arc<dyn Adapter>> {
    ADAPTERS
        .iter()
        .find(|item| item.id == id)
        .map(|item| (item.create)(path))
}

fn incomplete_workbuddy_cli(id: &str, path: &str) -> bool {
    let path = PathBuf::from(path);
    id == "codebuddy"
        && path
            .to_string_lossy()
            .contains(".app/Contents/Resources/app.asar.unpacked/cli/bin/")
        && !path
            .parent()
            .is_some_and(|parent| parent.join("../dist/codebuddy.js").is_file())
}

fn connectable_cli_name(id: &str, path: &str) -> bool {
    if incomplete_workbuddy_cli(id, path) {
        return false;
    }
    let path = PathBuf::from(path);
    let Some(name) = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_ascii_lowercase)
    else {
        return false;
    };
    (id == "mimo" && name == "mimo")
        || ADAPTERS
            .iter()
            .find(|item| item.id == id)
            .is_some_and(|item| item.executable_names.contains(&name.as_str()))
}

fn scanned_cli(id: &str, candidate: &str) -> bool {
    localmux_discovery::scan().applications.iter().any(|app| {
        app.app_id == id
            && app.detection_source == "executable"
            && app.executable.as_deref() == Some(candidate)
            && connectable_cli_name(id, candidate)
    })
}

#[tauri::command]
async fn scan_apps() -> Result<localmux_discovery::ScanReport, String> {
    let mut report = tokio::task::spawn_blocking(localmux_discovery::scan)
        .await
        .map_err(|error| error.to_string())?;
    for app in &mut report.applications {
        if app.app_id == "codebuddy"
            && app.detection_source == "executable"
            && app
                .executable
                .as_deref()
                .is_some_and(|path| incomplete_workbuddy_cli("codebuddy", path))
        {
            app.diagnostics.push(
                "WorkBuddy 包内 CLI 缺少完整运行文件；需要官方独立 CodeBuddy Code CLI 才能验证模型"
                    .into(),
            );
        }
        if app.app_id == "mimo" && app.detection_source == "executable" {
            app.diagnostics
                .push("MiMo Code CLI 已安装；点击连接后才启动官方本机能力 API 并验证模型".into());
        }
        if app.detection_source == "executable"
            && app
                .executable
                .as_deref()
                .is_some_and(|path| connectable_cli_name(&app.app_id, path))
        {
            app.connectability = localmux_discovery::Connectability::SupportedVersionUnknown;
        }
    }
    Ok(report)
}

#[tauri::command]
async fn add_detected_app(
    control: tauri::State<'_, GatewayControl>,
    app_id: String,
    install_path: String,
) -> Result<(), String> {
    let id = app_id.clone();
    let path = install_path.clone();
    let valid = tokio::task::spawn_blocking(move || {
        localmux_discovery::scan().applications.iter().any(|app| {
            app.app_id == id
                && app.install_path == path
                && app.detection_source != "executable"
                && !matches!(
                    app.connectability,
                    localmux_discovery::Connectability::BrokenInstall
                )
        })
    })
    .await
    .map_err(|error| error.to_string())?;
    if !valid {
        return Err("请选择已扫描到的完整桌面应用".into());
    }
    localmux_storage::upsert_added_application(&control.state.pool, &app_id, &install_path)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn added_apps(
    control: tauri::State<'_, GatewayControl>,
) -> Result<Vec<(String, String)>, String> {
    localmux_storage::saved_added_applications(&control.state.pool)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn remove_detected_app(
    control: tauri::State<'_, GatewayControl>,
    install_path: String,
) -> Result<(), String> {
    localmux_storage::delete_added_application(&control.state.pool, &install_path)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn connect_app(
    control: tauri::State<'_, GatewayControl>,
    app_id: String,
    executable: String,
) -> Result<Vec<String>, String> {
    let id = app_id.clone();
    let candidate = executable.clone();
    let valid = tokio::task::spawn_blocking(move || scanned_cli(&id, &candidate))
        .await
        .map_err(|error| error.to_string())?;
    if !valid {
        return Err("请选择已扫描到的官方 CLI 可执行文件".into());
    }
    let adapter = adapter_for(&app_id, PathBuf::from(&executable)).ok_or("尚无稳定 Adapter")?;
    let models = adapter
        .list_models()
        .await
        .map_err(|error| error.to_string())?;
    localmux_storage::upsert_connection(&control.state.pool, &app_id, &app_id, &executable)
        .await
        .map_err(|error| error.to_string())?;
    control.state.register(adapter).await;
    control.state.seed_catalog(&app_id, models.clone()).await;
    control.state.refresh_quota_for(&app_id, true).await;
    Ok(models.into_iter().map(|model| model.id).collect())
}

#[tauri::command]
async fn cli_auth_status(app_id: String, executable: String) -> Result<String, String> {
    if app_id != "qoder" && app_id != "qoder-cn" && app_id != "codex" && app_id != "mimo" {
        return Err("该运行时没有可确认的 CLI 登录状态".into());
    }
    let id = app_id.clone();
    let candidate = executable.clone();
    let valid = tokio::task::spawn_blocking(move || scanned_cli(&id, &candidate))
        .await
        .map_err(|error| error.to_string())?;
    if !valid {
        return Err("请选择已扫描到的官方 CLI".into());
    }
    let mut command = tokio::process::Command::new(&executable);
    if app_id == "qoder" || app_id == "qoder-cn" {
        command.args(["status", "--output", "json"]);
    } else if app_id == "mimo" {
        command.args(["providers", "whoami", "--pure"]);
    } else {
        command.args(["login", "status"]);
    }
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| "官方 CLI 登录状态检查超时")?
    .map_err(|_| "无法启动官方 CLI 登录状态检查")?;
    if app_id == "qoder" || app_id == "qoder-cn" {
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|_| "Qoder 登录状态格式无效")?;
        return Ok(match value["logged_in"].as_bool() {
            Some(true) => "READY",
            Some(false) => "AUTH_REQUIRED",
            None => "ERROR",
        }
        .into());
    }
    let message = format!(
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .to_ascii_lowercase();
    if app_id == "mimo" {
        return Ok(if message.contains("not logged") {
            "AUTH_REQUIRED"
        } else if output.status.success() && message.contains("logged") {
            "READY"
        } else {
            "ERROR"
        }
        .into());
    }
    Ok(if output.status.success()
        && message.contains("logged in")
        && !message.contains("not logged in")
    {
        "READY"
    } else if message.contains("not logged in") {
        "AUTH_REQUIRED"
    } else {
        "ERROR"
    }
    .into())
}

#[tauri::command]
async fn cli_login(app_id: String, executable: String) -> Result<(), String> {
    if app_id != "qoder" && app_id != "qoder-cn" && app_id != "codex" && app_id != "mimo" {
        return Err("该 CLI 没有可用的官方登录命令".into());
    }
    let candidate = executable.clone();
    let id = app_id.clone();
    let valid = tokio::task::spawn_blocking(move || scanned_cli(&id, &candidate))
        .await
        .map_err(|error| error.to_string())?;
    if !valid {
        return Err("请选择已扫描到的官方 CLI".into());
    }
    let mut command = tokio::process::Command::new(&executable);
    if app_id == "mimo" {
        command.args(["providers", "login", "-p", "xiaomi"]);
    } else {
        command.arg("login");
    }
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "无法启动官方 CLI 登录流程")?;
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    Ok(())
}

#[tauri::command]
async fn connect_mimo(
    control: tauri::State<'_, GatewayControl>,
    api_key: String,
) -> Result<Vec<String>, String> {
    if api_key.trim() != api_key || api_key.is_empty() || api_key.len() > 4096 {
        return Err("API Key 格式无效".into());
    }
    let adapter = mimo_adapter(api_key.clone())?;
    let models = adapter
        .list_models()
        .await
        .map_err(|error| error.to_string())?;
    let store = mimo_store();
    let previous_key = store
        .get(MIMO_SECRET_REF)
        .map_err(|error| error.to_string())?;
    store
        .put(MIMO_SECRET_REF, &api_key)
        .map_err(|error| error.to_string())?;
    if let Err(error) = localmux_storage::upsert_connection(
        &control.state.pool,
        "mimo",
        "mimo",
        MIMO_CONNECTION_REF,
    )
    .await
    {
        match previous_key {
            Some(key) => store.put(MIMO_SECRET_REF, &key),
            None => store.delete(MIMO_SECRET_REF),
        }
        .map_err(|_| "连接保存失败，凭据恢复也失败".to_owned())?;
        return Err(error.to_string());
    }
    control.state.register(adapter).await;
    control.state.seed_catalog("mimo", models.clone()).await;
    Ok(models.into_iter().map(|model| model.id).collect())
}

#[tauri::command]
async fn connect_mimocode(
    control: tauri::State<'_, GatewayControl>,
    executable: String,
) -> Result<Vec<String>, String> {
    let candidate = executable.clone();
    let valid = tokio::task::spawn_blocking(move || scanned_cli("mimo", &candidate))
        .await
        .map_err(|error| error.to_string())?;
    if !valid {
        return Err("请选择已扫描到的 MiMo Code CLI".into());
    }
    let mut slot = control.mimocode.lock().await;
    if slot.is_some() {
        return Err("MiMo Code 已连接，请先断开".into());
    }
    let dir = tempfile::tempdir().map_err(|_| "无法创建 MiMo Code 临时目录")?;
    let mut child = tokio::process::Command::new(&executable)
        .args(["serve", "--pure", "--hostname", "127.0.0.1", "--port", "0"])
        .current_dir(dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "无法启动 MiMo Code 本机服务")?;
    let mut broad = None;
    for _ in 0..20 {
        if child
            .try_wait()
            .map_err(|_| "MiMo Code 服务状态不可用")?
            .is_some()
        {
            return Err("MiMo Code 本机服务启动失败".into());
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if let Some(token) = issue_mimocode_token(&executable, dir.path(), &[]).await? {
            broad = Some(token);
            break;
        }
    }
    let broad = broad.ok_or("MiMo Code 本机服务未就绪")?;
    let catalog = localmux_adapter_sdk::provider_http::HttpProvider::new(
        "mimocode",
        &broad.base,
        SecretString::from(broad.key.clone()),
    );
    let listed = match catalog {
        Ok(catalog) => catalog
            .list_models()
            .await
            .map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    };
    revoke_mimocode_token(&executable, dir.path(), &broad.id).await?;
    let allowed: Vec<String> = listed?
        .into_iter()
        .filter_map(|model| {
            let id = model.id.strip_prefix("mimocode/")?;
            (id.starts_with("mimo/") || id.starts_with("xiaomi/")).then(|| id.to_owned())
        })
        .collect();
    if allowed.is_empty() {
        return Err("MiMo Code 未报告可连接的 MiMo 模型，请在官方 CLI 检查登录".into());
    }
    let scoped = issue_mimocode_token(&executable, dir.path(), &allowed)
        .await?
        .ok_or("MiMo Code 本机服务未就绪")?;
    let adapter = localmux_adapter_sdk::provider_http::HttpProvider::new(
        "mimocode",
        &scoped.base,
        SecretString::from(scoped.key),
    );
    let adapter = match adapter {
        Ok(adapter) => adapter,
        Err(error) => {
            let _ = revoke_mimocode_token(&executable, dir.path(), &scoped.id).await;
            return Err(error.to_string());
        }
    };
    let models = adapter
        .list_models()
        .await
        .map_err(|error| error.to_string());
    if models.is_err() {
        let _ = revoke_mimocode_token(&executable, dir.path(), &scoped.id).await;
    }
    let models = models?;
    control.state.register(Arc::new(adapter)).await;
    control.state.seed_catalog("mimocode", models.clone()).await;
    *slot = Some(MimoCodeSession {
        executable,
        _child: child,
        _dir: dir,
        token_id: scoped.id,
    });
    Ok(models.into_iter().map(|model| model.id).collect())
}

#[tauri::command]
async fn connected_apps(
    control: tauri::State<'_, GatewayControl>,
) -> Result<Vec<(String, String)>, String> {
    let mut connections = localmux_storage::saved_connections(&control.state.pool)
        .await
        .map_err(|error| error.to_string())?;
    if connections.iter().any(|(id, _)| id == "mimo")
        && mimo_store().get(MIMO_SECRET_REF).ok().flatten().is_none()
    {
        connections.retain(|(id, _)| id != "mimo");
    }
    connections.retain(|(id, path)| !incomplete_workbuddy_cli(id, path));
    if let Some(session) = control.mimocode.lock().await.as_ref() {
        connections.push(("mimocode".into(), session.executable.clone()));
    }
    Ok(connections)
}

#[tauri::command]
async fn disconnect_app(
    control: tauri::State<'_, GatewayControl>,
    app_id: String,
) -> Result<(), String> {
    if app_id == "mimocode" {
        let session = control
            .mimocode
            .lock()
            .await
            .take()
            .ok_or("MiMo Code 未连接")?;
        control.state.unregister("mimocode").await;
        if let Err(error) =
            revoke_mimocode_token(&session.executable, session._dir.path(), &session.token_id).await
        {
            tracing::warn!(%error, "MiMo Code token will expire within 30 minutes");
        }
        drop(session);
        return Ok(());
    }
    if app_id != "mimo" && adapter_for(&app_id, PathBuf::new()).is_none() {
        return Err("未知 Adapter".into());
    }
    if app_id == "mimo" {
        mimo_store()
            .delete(MIMO_SECRET_REF)
            .map_err(|error| error.to_string())?;
        control.state.unregister(&app_id).await;
    }
    localmux_storage::delete_connection(&control.state.pool, &app_id)
        .await
        .map_err(|error| error.to_string())?;
    if app_id != "mimo" {
        control.state.unregister(&app_id).await;
    }
    Ok(())
}

#[tauri::command]
async fn routing_get(control: tauri::State<'_, GatewayControl>) -> Result<RoutePolicy, String> {
    Ok(control.state.route_policy().await)
}

#[tauri::command]
async fn routing_set(
    control: tauri::State<'_, GatewayControl>,
    policy: RoutePolicy,
) -> Result<(), String> {
    if policy.priority.len() > 64
        || policy.best_model.as_deref().is_some_and(|id| {
            !id.contains('/') || matches!(id, "free" | "cheap" | "balanced" | "best")
        })
    {
        return Err("路由策略无效".into());
    }
    let json = serde_json::to_string(&policy).map_err(|error| error.to_string())?;
    localmux_storage::save_route_policy(&control.state.pool, "default", &json)
        .await
        .map_err(|error| error.to_string())?;
    control.state.set_route_policy(policy).await;
    Ok(())
}

#[tauri::command]
async fn routing_status(
    control: tauri::State<'_, GatewayControl>,
) -> Result<Vec<(String, Option<String>)>, String> {
    Ok(control.state.route_status().await)
}

#[tauri::command]
async fn routing_refresh(
    control: tauri::State<'_, GatewayControl>,
) -> Result<Vec<(String, Option<String>)>, String> {
    control.state.invalidate_all_catalog().await;
    control.state.refresh_quotas(false).await;
    Ok(control.state.route_status().await)
}

#[tauri::command]
async fn quota_snapshots(
    control: tauri::State<'_, GatewayControl>,
) -> Result<Vec<localmux_quota::QuotaSnapshot>, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs() as i64;
    let mut connections = localmux_storage::saved_connections(&control.state.pool)
        .await
        .map_err(|error| error.to_string())?;
    if connections.iter().any(|(id, _)| id == "mimo")
        && mimo_store().get(MIMO_SECRET_REF).ok().flatten().is_none()
    {
        connections.retain(|(id, _)| id != "mimo");
    }
    Ok(connections
        .into_iter()
        .map(|(id, _)| {
            control
                .state
                .quota_snapshot(&id)
                .filter(|snapshot| snapshot.observed_at <= now && now - snapshot.observed_at <= 600)
                .unwrap_or(localmux_quota::QuotaSnapshot {
                    connection_id: id,
                    remaining: None,
                    limit: None,
                    unit: None,
                    reset_at: None,
                    source: localmux_quota::QuotaSource::Unknown,
                    observed_at: 0,
                })
        })
        .collect())
}

#[tauri::command]
async fn quota_buckets(
    control: tauri::State<'_, GatewayControl>,
) -> Result<Vec<localmux_quota::QuotaBucket>, String> {
    localmux_storage::load_quota_buckets(&control.state.pool)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn sessions_list(
    control: tauri::State<'_, GatewayControl>,
) -> Result<Vec<localmux_storage::ConversationState>, String> {
    localmux_storage::list_conversations(&control.state.pool)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn session_events(
    control: tauri::State<'_, GatewayControl>,
    conversation_id: String,
) -> Result<Vec<localmux_storage::RunEvent>, String> {
    localmux_storage::list_run_events(&control.state.pool, &conversation_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn gateway_start(control: tauri::State<'_, GatewayControl>) -> Result<(), String> {
    start_gateway(&control).await
}
async fn start_gateway(control: &GatewayControl) -> Result<(), String> {
    let mut task = control.task.lock().await;
    if task.as_ref().is_some_and(|task| !task.is_finished()) {
        return Ok(());
    }
    let listener = tokio::net::TcpListener::bind((
        std::net::Ipv4Addr::LOCALHOST,
        localmux_gateway::DEFAULT_PORT,
    ))
    .await
    .map_err(|error| error.to_string())?;
    ensure_default_api_key(control).await?;
    let state = control.state.clone();
    *task = Some(tokio::spawn(async move {
        if let Err(error) = localmux_gateway::serve_on(listener, state).await {
            tracing::error!(%error, "gateway stopped unexpectedly");
        }
    }));
    Ok(())
}

#[tauri::command]
async fn gateway_stop(control: tauri::State<'_, GatewayControl>) -> Result<(), String> {
    stop_gateway(&control).await
}
async fn stop_gateway(control: &GatewayControl) -> Result<(), String> {
    if let Some(task) = control.task.lock().await.take() {
        task.abort();
    }
    Ok(())
}

#[tauri::command]
async fn gateway_status(control: tauri::State<'_, GatewayControl>) -> Result<bool, String> {
    Ok(control
        .task
        .lock()
        .await
        .as_ref()
        .is_some_and(|task| !task.is_finished()))
}

async fn create_stored_default_key(
    control: &GatewayControl,
    old_id: Option<&str>,
    read_only_agent: bool,
) -> Result<(), String> {
    let mut policy = localmux_security::KeyRestrictions::default();
    let mut scopes = vec!["models:read", "chat:invoke", "responses:invoke"];
    if read_only_agent {
        policy.execution_profile = localmux_security::ExecutionProfile::ReadOnlyAgent;
        scopes.push("agent:invoke");
    }
    let (id, key) = control
        .state
        .create_key_with_policy(&scopes, policy)
        .await
        .map_err(|error| error.to_string())?;
    let reference = default_key_ref(&id);
    let store = default_key_store();
    if let Err(error) = store.put(&reference, &key) {
        let _ = localmux_storage::revoke_api_key(&control.state.pool, &id).await;
        return Err(error.to_string());
    }
    if let Err(error) =
        localmux_storage::replace_default_api_key(&control.state.pool, &id, old_id).await
    {
        let _ = store.delete(&reference);
        let _ = localmux_storage::revoke_api_key(&control.state.pool, &id).await;
        return Err(error.to_string());
    }
    if let Some(old_id) = old_id {
        if let Err(error) = store.delete(&default_key_ref(old_id)) {
            tracing::warn!(%error, "revoked old API key remains in credential store");
        }
    }
    Ok(())
}

async fn default_key_material(
    control: &GatewayControl,
) -> Result<Option<(String, String)>, String> {
    let Some(id) = localmux_storage::setting_string(&control.state.pool, DEFAULT_KEY_SETTING)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let Some(key) = default_key_store()
        .get(&default_key_ref(&id))
        .map_err(|error| error.to_string())?
    else {
        return Err("本机 API Key 凭据已丢失，请显式重建".into());
    };
    if !control
        .state
        .key_matches_id(&id, &key)
        .await
        .map_err(|error| error.to_string())?
    {
        return Err("本机 API Key 已失效，请显式重建".into());
    }
    Ok(Some((id, key)))
}

async fn ensure_default_api_key(control: &GatewayControl) -> Result<(), String> {
    let _guard = control.default_key_lock.lock().await;
    let old_id = localmux_storage::setting_string(&control.state.pool, DEFAULT_KEY_SETTING)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(id) = &old_id {
        if default_key_store()
            .get(&default_key_ref(id))
            .map_err(|error| error.to_string())?
            .is_some()
        {
            default_key_material(control).await?;
            return Ok(());
        }
    }
    create_stored_default_key(control, old_id.as_deref(), false).await
}

#[tauri::command]
async fn default_api_key_status(
    control: tauri::State<'_, GatewayControl>,
) -> Result<String, String> {
    Ok(match default_key_material(&control).await {
        Ok(Some(_)) => "READY",
        Ok(None) => "ABSENT",
        Err(_) => "MISSING",
    }
    .into())
}

#[tauri::command]
async fn default_api_key_allows_agent(
    control: tauri::State<'_, GatewayControl>,
) -> Result<bool, String> {
    let id = localmux_storage::setting_string(&control.state.pool, DEFAULT_KEY_SETTING)
        .await
        .map_err(|error| error.to_string())?;
    let keys = localmux_storage::list_api_keys(&control.state.pool)
        .await
        .map_err(|error| error.to_string())?;
    Ok(keys.iter().any(|key| {
        Some(key.id.as_str()) == id.as_deref()
            && key.scopes.iter().any(|scope| scope == "agent:invoke")
    }))
}

#[tauri::command]
async fn reveal_default_api_key(
    control: tauri::State<'_, GatewayControl>,
) -> Result<String, String> {
    default_key_material(&control)
        .await?
        .map(|(_, key)| key)
        .ok_or_else(|| "请先启动 API".to_owned())
}

#[tauri::command]
async fn reset_default_api_key(
    control: tauri::State<'_, GatewayControl>,
    read_only_agent: bool,
) -> Result<(), String> {
    let _guard = control.default_key_lock.lock().await;
    let old_id = localmux_storage::setting_string(&control.state.pool, DEFAULT_KEY_SETTING)
        .await
        .map_err(|error| error.to_string())?;
    create_stored_default_key(&control, old_id.as_deref(), read_only_agent).await
}

#[tauri::command]
async fn api_setup_models(
    control: tauri::State<'_, GatewayControl>,
) -> Result<Vec<String>, String> {
    if !control
        .task
        .lock()
        .await
        .as_ref()
        .is_some_and(|task| !task.is_finished())
    {
        return Err("请先启动 API".into());
    }
    let (_, key) = default_key_material(&control)
        .await?
        .ok_or("本机 API Key 尚未创建")?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(|_| "无法请求本机 API")?;
    let response = client
        .get(gateway_base_url() + "/models")
        .bearer_auth(key)
        .send()
        .await
        .map_err(|_| "本机 API 未响应")?;
    if !response.status().is_success() {
        return Err("本机模型目录不可用，请检查 Key 与服务状态".into());
    }
    let value: serde_json::Value = response.json().await.map_err(|_| "本机模型目录格式无效")?;
    let models = value["data"].as_array().ok_or("本机模型目录格式无效")?;
    Ok(models
        .iter()
        .filter_map(|item| item["id"].as_str())
        .filter(|id| id.contains('/'))
        .map(str::to_owned)
        .collect())
}
#[tauri::command]
async fn verify_model(
    control: tauri::State<'_, GatewayControl>,
    model: String,
) -> Result<u64, String> {
    let started = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(120),
        control.state.verify_reported_model(&model),
    )
    .await
    .map_err(|_| "模型测试超时".to_owned())?
    .map_err(|error| error.to_string())?;
    Ok(started.elapsed().as_millis() as u64)
}

#[tauri::command]
fn gateway_base_url() -> String {
    format!("http://127.0.0.1:{}/v1", localmux_gateway::DEFAULT_PORT)
}

#[tauri::command]
fn autostart_status(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn autostart_set(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn close_to_tray_status(control: tauri::State<'_, GatewayControl>) -> bool {
    control.close_to_tray.load(Ordering::Relaxed)
}

#[tauri::command]
async fn close_to_tray_set(
    control: tauri::State<'_, GatewayControl>,
    enabled: bool,
) -> Result<(), String> {
    localmux_storage::save_setting_bool(&control.state.pool, "close_to_tray", enabled)
        .await
        .map_err(|error| error.to_string())?;
    control.close_to_tray.store(enabled, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn updater_available() -> bool {
    option_env!("LOCALMUX_UPDATER_PUBLIC_KEY").is_some_and(|value| !value.is_empty())
        && option_env!("LOCALMUX_UPDATER_ENDPOINT")
            .is_some_and(|value| value.starts_with("https://"))
}

#[tauri::command]
async fn updater_check(app: tauri::AppHandle) -> Result<Option<String>, String> {
    if !updater_available() {
        return Err("尚未配置可信更新源".into());
    }
    let endpoint = option_env!("LOCALMUX_UPDATER_ENDPOINT").ok_or("尚未配置更新源")?;
    let update = app
        .updater_builder()
        .endpoints(vec![endpoint.parse().map_err(|_| "更新源地址无效")?])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;
    Ok(update.map(|item| item.version))
}

#[tauri::command]
async fn updater_install(app: tauri::AppHandle, expected_version: String) -> Result<(), String> {
    if !updater_available() {
        return Err("尚未配置可信更新源".into());
    }
    let control = app.state::<GatewayControl>();
    if control.state.active_tasks() > 0 {
        return Err("请等待当前任务结束后再更新".into());
    }
    let endpoint = option_env!("LOCALMUX_UPDATER_ENDPOINT").ok_or("尚未配置更新源")?;
    let update = app
        .updater_builder()
        .endpoints(vec![endpoint.parse().map_err(|_| "更新源地址无效")?])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?
        .ok_or("当前已是最新版本")?;
    if update.version != expected_version {
        return Err("可用版本已变化，请重新检查更新".into());
    }
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
struct NewApiKey {
    id: String,
    key: String,
}

#[tauri::command]
async fn generate_api_key(
    control: tauri::State<'_, GatewayControl>,
    policy: localmux_security::KeyRestrictions,
) -> Result<NewApiKey, String> {
    if !policy.valid() {
        return Err("API Key 权限配置无效".into());
    }
    let scopes = if policy.execution_profile == localmux_security::ExecutionProfile::ReadOnlyAgent {
        vec![
            "models:read",
            "chat:invoke",
            "responses:invoke",
            "agent:invoke",
        ]
    } else {
        vec!["models:read", "chat:invoke", "responses:invoke"]
    };
    let (id, key) = control
        .state
        .create_key_with_policy(&scopes, policy)
        .await
        .map_err(|error| error.to_string())?;
    Ok(NewApiKey { id, key })
}

#[tauri::command]
async fn revoke_api_key(
    control: tauri::State<'_, GatewayControl>,
    id: String,
) -> Result<(), String> {
    localmux_storage::revoke_api_key(&control.state.pool, &id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn list_api_keys(
    control: tauri::State<'_, GatewayControl>,
) -> Result<Vec<localmux_storage::ApiKeySummary>, String> {
    localmux_storage::list_api_keys(&control.state.pool)
        .await
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    localmux_telemetry::init();
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_clipboard_manager::init())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let control = window.state::<GatewayControl>();
                if control.playground.busy() || control.state.active_tasks() > 0 {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event { api.prevent_close(); }
                    let _ = window.emit("localmux-exit-blocked", ());
                    return;
                }
                if control.close_to_tray.load(Ordering::Relaxed) {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") { native_material::install(&window); }
            if let Some(public_key) = option_env!("LOCALMUX_UPDATER_PUBLIC_KEY").filter(|value| !value.is_empty()) {
                if option_env!("LOCALMUX_UPDATER_ENDPOINT").is_some_and(|value| value.starts_with("https://")) {
                    app.handle().plugin(tauri_plugin_updater::Builder::new().pubkey(public_key).build())?;
                }
            }
            // Keep the existing data directory when the visible app name changes.
            let data_dir = localmux_core::data_dir("LocalMux")
                .ok_or_else(|| std::io::Error::other("application data directory unavailable"))?;
            let pool = tauri::async_runtime::block_on(localmux_storage::open(
                &data_dir.join("localmux.db"),
            ))?;
            let state = localmux_gateway::GatewayState::new(pool.clone());
            if let Some(json) = tauri::async_runtime::block_on(
                localmux_storage::load_route_policy(&pool, "default"),
            )? {
                match serde_json::from_str::<RoutePolicy>(&json) {
                    Ok(policy) => tauri::async_runtime::block_on(state.set_route_policy(policy)),
                    Err(error) => tracing::warn!(%error, "stored route policy ignored"),
                }
            }
            for snapshot in
                tauri::async_runtime::block_on(localmux_storage::load_quota_snapshots(&pool))?
            {
                state.restore_quota_snapshot(snapshot);
            }
            for (id, path) in
                tauri::async_runtime::block_on(localmux_storage::saved_connections(&pool))?
            {
                if incomplete_workbuddy_cli(&id, &path) { continue; }
                let adapter = if id == "mimo" && path == MIMO_CONNECTION_REF {
                    mimo_store()
                        .get(MIMO_SECRET_REF)
                        .ok()
                        .flatten()
                        .and_then(|key| mimo_adapter(key).ok())
                } else {
                    adapter_for(&id, PathBuf::from(path))
                };
                if let Some(adapter) = adapter {
                    tauri::async_runtime::block_on(state.register(adapter));
                }
            }
            app.manage(GatewayControl {
                state: state.clone(),
                task: Mutex::new(None),
                mimocode: Mutex::new(None),
                default_key_lock: Mutex::new(()),
                playground: playground::Control::default(),
                close_to_tray: AtomicBool::new(tauri::async_runtime::block_on(localmux_storage::setting_bool(&pool, "close_to_tray"))?.unwrap_or(false)),
            });
            tauri::async_runtime::block_on(ensure_default_api_key(&app.state::<GatewayControl>()))
                .map_err(std::io::Error::other)?;
            let gateway_status = MenuItem::with_id(app, "gateway_status", "API：未启动", false, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", "打开 LocalAiMux", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "启动 API", true, None::<&str>)?;
            let copy_url = MenuItem::with_id(app, "copy_url", "复制 API 地址", true, None::<&str>)?;
            let tasks = MenuItem::with_id(app, "tasks", "运行任务：0", false, None::<&str>)?;
            let warnings = MenuItem::with_id(app, "warnings", "警告：0", false, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出 LocalAiMux", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&gateway_status, &open, &toggle, &copy_url, &tasks, &warnings, &quit])?;
            let icon = app.default_window_icon().ok_or_else(|| std::io::Error::other("tray icon unavailable"))?.clone();
            TrayIconBuilder::new().icon(icon).tooltip("LocalAiMux").menu(&menu)
                .on_menu_event(|handle, event| match event.id.0.as_str() {
                    "open" => { if let Some(window) = handle.get_webview_window("main") { let _ = window.show(); let _ = window.set_focus(); } },
                    "toggle" => { let handle = handle.clone(); tauri::async_runtime::spawn(async move {
                        let control = handle.state::<GatewayControl>();
                        let active = control.task.lock().await.as_ref().is_some_and(|task| !task.is_finished());
                        let result = if active { stop_gateway(&control).await } else { start_gateway(&control).await };
                        if let Err(error) = result { tracing::warn!(message = %localmux_telemetry::redact(&error), "tray API toggle failed"); }
                    }); },
                    "copy_url" => { let _ = handle.clipboard().write_text(format!("http://127.0.0.1:{}/v1", localmux_gateway::DEFAULT_PORT)); },
                    "quit" => handle.exit(0),
                    _ => {},
                }).build(app)?;
            let tray_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let control = tray_app.state::<GatewayControl>();
                    let running = control.task.lock().await.as_ref().is_some_and(|task| !task.is_finished());
                    let _ = gateway_status.set_text(if running { "API：运行中" } else { "API：未启动" });
                    let _ = toggle.set_text(if running { "停止 API" } else { "启动 API" });
                    let _ = tasks.set_text(format!("运行任务：{}", control.state.active_tasks()));
                    let _ = warnings.set_text(format!("警告：{}", control.state.warning_count()));
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            });
            tauri::async_runtime::spawn(async move {
                state.refresh_quotas(true).await;
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                    state.refresh_quotas(false).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_apps,
            add_detected_app,
            added_apps,
            remove_detected_app,
            connect_app,
            cli_auth_status,
            cli_login,
            connect_mimo,
            connect_mimocode,
            connected_apps,
            disconnect_app,
            routing_get,
            routing_set,
            routing_status,
            routing_refresh,
            quota_snapshots,
            quota_buckets,
            sessions_list,
            session_events,
            gateway_start,
            gateway_stop,
            gateway_status,
            gateway_base_url,
            autostart_status,
            autostart_set,
            close_to_tray_status,
            close_to_tray_set,
            updater_available,
            updater_check,
            updater_install,
            generate_api_key,
            revoke_api_key,
            list_api_keys,
            default_api_key_status,
            default_api_key_allows_agent,
            reveal_default_api_key,
            reset_default_api_key,
            api_setup_models,
            playground::chat_models,
            playground::chat_send,
            playground::chat_cancel,
            native_material::native_appearance,
            native_material::native_theme,
            verify_model
        ])
        .build(tauri::generate_context!())
        .expect("failed to build LocalAiMux desktop application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if let Some(control) = app.try_state::<GatewayControl>() {
                    if control.playground.busy() || control.state.active_tasks() > 0 {
                        api.prevent_exit();
                        let _ = app.emit("localmux-exit-blocked", ());
                        if let Some(window) = app.get_webview_window("main") { let _ = window.show(); }
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::trusted_mimocode_base;

    #[test]
    fn mimocode_token_never_goes_to_a_non_loopback_endpoint() {
        assert!(trusted_mimocode_base("http://127.0.0.1:4567/v1"));
        for base in [
            "https://example.com/v1",
            "http://localhost:4567/v1",
            "http://127.0.0.1:4567/other",
            "http://user@127.0.0.1:4567/v1",
        ] {
            assert!(!trusted_mimocode_base(base));
        }
    }
}
