use localmux_adapter_sdk::Adapter;
use localmux_router::RoutePolicy;
use localmux_secrets::{OsSecretStore, SecretStore};
use secrecy::SecretString;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::Manager;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Mutex;

struct GatewayControl {
    state: localmux_gateway::GatewayState,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    close_to_tray: AtomicBool,
}

const MIMO_BASE_URL: &str = "https://api.xiaomimimo.com/v1";
const MIMO_SECRET_REF: &str = "provider.mimo";
const MIMO_CONNECTION_REF: &str = "os-credential:mimo";

fn mimo_store() -> OsSecretStore {
    OsSecretStore::new("app.localmux.desktop")
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

fn adapter_for(id: &str, path: PathBuf) -> Option<Arc<dyn Adapter>> {
    match id {
        "codex" => Some(Arc::new(localmux_adapter_codex::CodexAdapter::new(path))),
        "qoder" => Some(Arc::new(localmux_adapter_qoder::QoderAdapter::new(path))),
        "codebuddy" => Some(Arc::new(localmux_adapter_codebuddy::CodeBuddyAdapter::new(
            path,
        ))),
        _ => None,
    }
}

fn connectable_cli_name(id: &str, path: &str) -> bool {
    let path = PathBuf::from(path);
    let Some(name) = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_ascii_lowercase)
    else {
        return false;
    };
    match id {
        "codex" => name == "codex" || name == "codex.exe",
        "qoder" => name == "qoder" || name == "qoder.exe",
        "codebuddy" => name == "codebuddy" || name == "codebuddy.exe",
        _ => false,
    }
}

#[tauri::command]
async fn scan_apps() -> Result<localmux_discovery::ScanReport, String> {
    let mut report = tokio::task::spawn_blocking(localmux_discovery::scan)
        .await
        .map_err(|error| error.to_string())?;
    for app in &mut report.applications {
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
    let valid = tokio::task::spawn_blocking(move || {
        localmux_discovery::scan().applications.iter().any(|app| {
            app.app_id == id
                && app.detection_source == "executable"
                && app.executable.as_deref() == Some(candidate.as_str())
                && connectable_cli_name(&id, &candidate)
        })
    })
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
    Ok(connections)
}

#[tauri::command]
async fn disconnect_app(
    control: tauri::State<'_, GatewayControl>,
    app_id: String,
) -> Result<(), String> {
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
                if control.close_to_tray.load(Ordering::Relaxed) {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .setup(|app| {
            if let Some(public_key) = option_env!("LOCALMUX_UPDATER_PUBLIC_KEY").filter(|value| !value.is_empty()) {
                if option_env!("LOCALMUX_UPDATER_ENDPOINT").is_some_and(|value| value.starts_with("https://")) {
                    app.handle().plugin(tauri_plugin_updater::Builder::new().pubkey(public_key).build())?;
                }
            }
            let product_name = app
                .config()
                .product_name
                .as_deref()
                .ok_or_else(|| std::io::Error::other("product name unavailable"))?;
            let data_dir = localmux_core::data_dir(product_name)
                .ok_or_else(|| std::io::Error::other("application data directory unavailable"))?;
            let pool = tauri::async_runtime::block_on(localmux_storage::open(
                &data_dir.join("localmux.db"),
            ))?;
            let state = localmux_gateway::GatewayState::mock(pool.clone());
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
                close_to_tray: AtomicBool::new(tauri::async_runtime::block_on(localmux_storage::setting_bool(&pool, "close_to_tray"))?.unwrap_or(false)),
            });
            let gateway_status = MenuItem::with_id(app, "gateway_status", "API：未启动", false, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", "打开智汇枢", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "启动 API", true, None::<&str>)?;
            let copy_url = MenuItem::with_id(app, "copy_url", "复制 API 地址", true, None::<&str>)?;
            let tasks = MenuItem::with_id(app, "tasks", "运行任务：0", false, None::<&str>)?;
            let warnings = MenuItem::with_id(app, "warnings", "警告：0", false, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出智汇枢", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&gateway_status, &open, &toggle, &copy_url, &tasks, &warnings, &quit])?;
            let icon = app.default_window_icon().ok_or_else(|| std::io::Error::other("tray icon unavailable"))?.clone();
            TrayIconBuilder::new().icon(icon).tooltip("LocalMux · 智汇枢").menu(&menu)
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
            connect_mimo,
            connected_apps,
            disconnect_app,
            routing_get,
            routing_set,
            routing_status,
            routing_refresh,
            quota_snapshots,
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
            list_api_keys
        ])
        .run(tauri::generate_context!())
        .expect("failed to run LocalMux desktop application");
}
