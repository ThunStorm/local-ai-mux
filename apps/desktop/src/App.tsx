import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import config from "../src-tauri/tauri.conf.json";
import "./App.css";

type Locale = "zh" | "en";
type Theme = "system" | "light" | "dark";
type View = "home" | "discover" | "routing" | "sessions" | "api";
type Detection = { app_id: string; name: string; version: string | null; executable: string | null; install_path: string; detection_source: string; connectability: string; diagnostics: string[] };
type ScanReport = { applications: Detection[]; scanned_roots: string[] };
type NewApiKey = { id: string; key: string };
type KeyPolicy = { allowed_routes: string[]; allowed_connections: string[]; execution_profile: "INFERENCE_ONLY" | "READ_ONLY_AGENT"; rate_limit_per_minute: number };
type KeySummary = { id: string; scopes: string[]; policy: KeyPolicy };
type RoutePolicy = { reserve_as_free: boolean; allow_metered: boolean; best_model: string | null; priority: string[] };
type QuotaSnapshot = { connection_id: string; remaining: number | null; limit: number | null; unit: string | null; reset_at: number | null; source: string; observed_at: number };
type Conversation = { id: string; model_id: string; adapter_id: string; provider_session_id: string | null; resume_supported: boolean; created_at: number; last_active_at: number; status: string };
type RunEvent = { kind: string; detail_json: string; observed_at: number };
const defaultPolicy: RoutePolicy = { reserve_as_free: false, allow_metered: false, best_model: null, priority: [] };
const defaultKeyPolicy: KeyPolicy = { allowed_routes: [], allowed_connections: [], execution_profile: "INFERENCE_ONLY", rate_limit_per_minute: 60 };
const isConnectable = (app: Detection) => app.connectability === "SUPPORTED_VERSION_UNKNOWN" && !!app.executable;
const isDesktopAddable = (app: Detection) => app.detection_source !== "executable" && app.connectability !== "BROKEN_INSTALL";
const quotaGuides: Record<string, string> = { qoder: "https://docs.qoder.com/cli/usage", codebuddy: "https://www.codebuddy.cn/docs/workbuddy/Credits", mimo: "https://platform.xiaomimimo.com/" };
const copy = {
  zh: { home: "总览", discover: "发现", routing: "路由", sessions: "会话", api: "API", welcome: "欢迎使用智汇枢", intro: "扫描已安装的 Agent，查看可接入状态，再按阶段连接你的账号。", phase: "当前阶段", phaseValue: "Phase 8 · 安全与打包", detected: "检测结果", scan: "重新扫描", scanning: "扫描中…", none: "未发现目标应用", roots: "已检查的位置", diagnostics: "诊断信息", detectedOnly: "仅检测到，尚未连接", broken: "安装不完整", add: "添加", addDesktop: "添加应用", addedState: "已添加应用（仅登记）", removeDesktop: "移除应用", pending: "该 Adapter 尚未接入；后续阶段开放连接", firstRun: "首次使用", steps: "欢迎 → 扫描 → 连接 → 开启 API", later: "稍后继续", apiTitle: "本地 API", running: "运行中", stopped: "未启动", start: "启动 API", stop: "停止 API", createKey: "创建 API Key", keyOnce: "请立即复制。此 Key 仅显示一次，数据库只存哈希。", copy: "复制", copied: "已复制", revoke: "撤销此 Key", language: "语言", theme: "外观", system: "跟随系统", light: "浅色", dark: "深色", unavailable: "桌面服务不可用", connected: "已连接服务", disconnect: "断开", connecting: "连接中…", connectedState: "已连接", loginHint: "请先在对应官方 CLI 完成登录，再重试连接", versionUnverified: "CLI 版本未验证", routeTitle: "虚拟路由", refresh: "刷新目录与额度", unavailableRoute: "暂无符合条件的模型", reserveFree: "允许已确认可用的 Reserve 进入 free", metered: "balanced 允许计费模型（可能产生费用）", bestModel: "best 指定模型 ID", priority: "优先级模型 ID（逗号分隔）", save: "保存路由策略", saving: "保存中…", quota: "官方额度", quotaUnknown: "上游尚未报告额度，状态未知", providerReported: "上游报告", estimated: "估算", unknown: "未知", sessionTitle: "会话记录", sessionEmpty: "暂无会话。只有已完成的 Responses 调用会出现在这里。", sessionResume: "支持上游续接", sessionNoResume: "上游不支持续接", sessionEvents: "路由事件", noEvents: "暂无路由事件", lastActive: "最近活动", sessionRefresh: "刷新会话", profile: "执行权限", inferenceOnly: "仅推理（默认）", readOnlyAgent: "只读 Agent", agentWarning: "只读 Agent 允许调用已连接的官方 CLI。LocalMux 会请求只读模式，但上游运行时仍自行管理工具权限。", allowedRoutes: "允许的模型 / 虚拟路由（逗号分隔，留空为全部）", allowedConnections: "允许的连接 ID（逗号分隔，留空为全部）", rateLimit: "每分钟请求上限" },
  en: { home: "Overview", discover: "Discover", routing: "Routing", sessions: "Sessions", api: "API", welcome: "Welcome to LocalMux", intro: "Find installed agents, inspect their status, then connect accounts in later phases.", phase: "Current phase", phaseValue: "Phase 8 · Security", detected: "Detected apps", scan: "Rescan", scanning: "Scanning…", none: "No target apps found", roots: "Locations checked", diagnostics: "Diagnostics", detectedOnly: "Detected, not connected", broken: "Incomplete installation", add: "Add", addDesktop: "Add app", addedState: "Added app (inventory only)", removeDesktop: "Remove app", pending: "This adapter is not available yet", firstRun: "First run", steps: "Welcome → Scan → Connect → Start API", later: "Continue later", apiTitle: "Local API", running: "Running", stopped: "Stopped", start: "Start API", stop: "Stop API", createKey: "Create API key", keyOnce: "Copy now. This key is shown only once; the database stores its hash.", copy: "Copy", copied: "Copied", revoke: "Revoke this key", language: "Language", theme: "Appearance", system: "System", light: "Light", dark: "Dark", unavailable: "Desktop service unavailable", connected: "Connections", disconnect: "Disconnect", connecting: "Connecting…", connectedState: "Connected", loginHint: "Sign in through the official CLI, then retry", versionUnverified: "CLI version unverified", routeTitle: "Virtual routes", refresh: "Refresh catalog and quota", unavailableRoute: "No eligible model", reserveFree: "Allow confirmed available Reserve in free", metered: "Allow metered models in balanced (may incur charges)", bestModel: "Explicit model ID for best", priority: "Priority model IDs (comma separated)", save: "Save route policy", saving: "Saving…", quota: "Official quota", quotaUnknown: "No upstream quota report; status unknown", providerReported: "Provider reported", estimated: "Estimated", unknown: "Unknown", sessionTitle: "Conversations", sessionEmpty: "No sessions yet. Completed Responses calls appear here.", sessionResume: "Upstream resume available", sessionNoResume: "Upstream resume unavailable", sessionEvents: "Routing events", noEvents: "No routing events", lastActive: "Last active", sessionRefresh: "Refresh sessions", profile: "Execution profile", inferenceOnly: "Inference only (default)", readOnlyAgent: "Read-only agent", agentWarning: "Read-only agent calls connected official CLIs. LocalMux requests read-only mode; the upstream runtime controls its own tool permissions.", allowedRoutes: "Allowed model / virtual route IDs (comma separated; blank = all)", allowedConnections: "Allowed connection IDs (comma separated; blank = all)", rateLimit: "Requests per minute" },
};

function App() {
  const [locale, setLocale] = useState<Locale>(() => localStorage.getItem("localmux.locale") === "en" ? "en" : "zh");
  const [theme, setTheme] = useState<Theme>(() => { const value = localStorage.getItem("localmux.theme"); return value === "light" || value === "dark" ? value : "system"; });
  const [view, setView] = useState<View>("home");
  const [firstRun, setFirstRun] = useState(() => localStorage.getItem("localmux.seen") !== "yes");
  const [report, setReport] = useState<ScanReport | null>(null);
  const [scanning, setScanning] = useState(false);
  const [running, setRunning] = useState(false);
  const [baseUrl, setBaseUrl] = useState("");
  const [policy, setPolicy] = useState<RoutePolicy>(defaultPolicy);
  const [routeStatus, setRouteStatus] = useState<[string, string | null][]>([]);
  const [quotaSnapshots, setQuotaSnapshots] = useState<QuotaSnapshot[]>([]);
  const [routeBusy, setRouteBusy] = useState(false);
  const [sessions, setSessions] = useState<Conversation[]>([]);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [sessionEvents, setSessionEvents] = useState<RunEvent[]>([]);
  const [connectedApps, setConnectedApps] = useState<[string, string][]>([]);
  const [addedApps, setAddedApps] = useState<[string, string][]>([]);
  const [addingApp, setAddingApp] = useState<string | null>(null);
  const [selectedModels, setSelectedModels] = useState<Record<string, string[]>>({});
  const [connecting, setConnecting] = useState<string | null>(null);
  const [mimoKey, setMimoKey] = useState("");
  const [key, setKey] = useState<NewApiKey | null>(null);
  const [keys, setKeys] = useState<KeySummary[]>([]);
  const [keyPolicy, setKeyPolicy] = useState<KeyPolicy>(defaultKeyPolicy);
  const [keyRoutesText, setKeyRoutesText] = useState("");
  const [keyConnectionsText, setKeyConnectionsText] = useState("");
  const [autoStart, setAutoStart] = useState(false);
  const [closeToTray, setCloseToTray] = useState(false);
  const [updaterReady, setUpdaterReady] = useState(false);
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState("");
  const t = copy[locale];

  useEffect(() => { localStorage.setItem("localmux.locale", locale); document.documentElement.lang = locale === "zh" ? "zh-CN" : "en"; }, [locale]);
  useEffect(() => { localStorage.setItem("localmux.theme", theme); document.documentElement.dataset.theme = theme; }, [theme]);
  useEffect(() => {
    let active = true;
    invoke<ScanReport>("scan_apps").then(value => { if (active) setReport(value); }).catch(() => { if (active) setError(copy[locale].unavailable); });
    invoke<boolean>("gateway_status").then(value => { if (active) setRunning(value); }).catch(() => {});
    invoke<string>("gateway_base_url").then(value => { if (active) setBaseUrl(value); }).catch(() => {});
    invoke<[string, string][]>("connected_apps").then(value => { if (active) setConnectedApps(value); }).catch(() => {});
    invoke<[string, string][]>("added_apps").then(value => { if (active) setAddedApps(value); }).catch(() => {});
    invoke<RoutePolicy>("routing_get").then(value => { if (active) setPolicy(value); }).catch(() => {});
    invoke<[string, string | null][]>("routing_status").then(value => { if (active) setRouteStatus(value); }).catch(() => {});
    invoke<QuotaSnapshot[]>("quota_snapshots").then(value => { if (active) setQuotaSnapshots(value); }).catch(() => {});
    invoke<Conversation[]>("sessions_list").then(value => { if (active) setSessions(value); }).catch(() => {});
    invoke<boolean>("autostart_status").then(value => { if (active) setAutoStart(value); }).catch(() => {});
    invoke<boolean>("close_to_tray_status").then(value => { if (active) setCloseToTray(value); }).catch(() => {});
    invoke<boolean>("updater_available").then(value => { if (active) setUpdaterReady(value); }).catch(() => {});
    invoke<KeySummary[]>("list_api_keys").then(value => { if (active) setKeys(value); }).catch(() => {});
    return () => { active = false; };
  }, []);

  async function rescan() {
    setScanning(true); setError("");
    try { setReport(await invoke<ScanReport>("scan_apps")); }
    catch (cause) { setError(String(cause)); }
    finally { setScanning(false); }
  }
  async function toggleGateway() {
    setError("");
    try { await invoke(running ? "gateway_stop" : "gateway_start"); setRunning(await invoke<boolean>("gateway_status")); }
    catch (cause) { setError(String(cause)); }
  }
  async function connectApp(appId: string, executable: string) {
    setConnecting(appId); setError("");
    try {
      const models = await invoke<string[]>("connect_app", { appId, executable });
      setSelectedModels(current => ({ ...current, [appId]: models }));
      setConnectedApps(current => [...current.filter(([id]) => id !== appId), [appId, executable]]);
      setRouteStatus(await invoke<[string, string | null][]>("routing_status"));
      setQuotaSnapshots(await invoke<QuotaSnapshot[]>("quota_snapshots"));
    } catch (cause) {
      const message = String(cause);
      setError(locale === "zh" && message.includes("authentication required") ? t.loginHint : message);
    } finally { setConnecting(null); }
  }
  async function connectMimo() {
    setConnecting("mimo"); setError("");
    try {
      const models = await invoke<string[]>("connect_mimo", { apiKey: mimoKey });
      setMimoKey("");
      setSelectedModels(current => ({ ...current, mimo: models }));
      setConnectedApps(current => [...current.filter(([id]) => id !== "mimo"), ["mimo", "os-credential:mimo"]]);
      setRouteStatus(await invoke<[string, string | null][]>("routing_status"));
      setQuotaSnapshots(await invoke<QuotaSnapshot[]>("quota_snapshots"));
    } catch (cause) { setError(String(cause)); }
    finally { setConnecting(null); }
  }
  async function disconnectApp(appId: string) {
    setConnecting(appId); setError("");
    try {
      await invoke("disconnect_app", { appId });
      setConnectedApps(current => current.filter(([id]) => id !== appId));
      setSelectedModels(current => { const next = { ...current }; delete next[appId]; return next; });
      setRouteStatus(await invoke<[string, string | null][]>("routing_status"));
      setQuotaSnapshots(await invoke<QuotaSnapshot[]>("quota_snapshots"));
    } catch (cause) { setError(String(cause)); }
    finally { setConnecting(null); }
  }
  async function toggleDetectedApp(appId: string, installPath: string) {
    setAddingApp(installPath); setError("");
    const exists = addedApps.some(([id, path]) => id === appId && path === installPath);
    try {
      await invoke(exists ? "remove_detected_app" : "add_detected_app", exists ? { installPath } : { appId, installPath });
      setAddedApps(await invoke<[string, string][]>("added_apps"));
    } catch (cause) { setError(String(cause)); }
    finally { setAddingApp(null); }
  }
  async function saveRouting() {
    setRouteBusy(true); setError("");
    try {
      const next = { ...policy, best_model: policy.best_model?.trim() || null, priority: policy.priority.filter(Boolean) };
      await invoke("routing_set", { policy: next });
      setPolicy(next);
      setRouteStatus(await invoke<[string, string | null][]>("routing_status"));
    } catch (cause) { setError(String(cause)); }
    finally { setRouteBusy(false); }
  }
  async function refreshRouting() {
    setRouteBusy(true); setError("");
    try {
      setRouteStatus(await invoke<[string, string | null][]>("routing_refresh"));
      setQuotaSnapshots(await invoke<QuotaSnapshot[]>("quota_snapshots"));
    } catch (cause) { setError(String(cause)); }
    finally { setRouteBusy(false); }
  }
  async function refreshSessions() {
    setError("");
    try { setSessions(await invoke<Conversation[]>("sessions_list")); }
    catch (cause) { setError(String(cause)); }
  }
  async function showSessionEvents(id: string) {
    setSelectedSession(id); setError("");
    try { setSessionEvents(await invoke<RunEvent[]>("session_events", { conversationId: id })); }
    catch (cause) { setError(String(cause)); }
  }
  async function createKey() {
    setError(""); setCopied(false);
    try { const policy = { ...keyPolicy, allowed_routes: keyRoutesText.split(",").map(item => item.trim()).filter(Boolean), allowed_connections: keyConnectionsText.split(",").map(item => item.trim()).filter(Boolean) }; setKey(await invoke<NewApiKey>("generate_api_key", { policy })); setKeys(await invoke<KeySummary[]>("list_api_keys")); }
    catch (cause) { setError(String(cause)); }
  }
  async function revokeKey(id: string) {
    try { await invoke("revoke_api_key", { id }); if (key?.id === id) setKey(null); setKeys(await invoke<KeySummary[]>("list_api_keys")); }
    catch (cause) { setError(String(cause)); }
  }
  async function copyText(value: string) {
    try { await navigator.clipboard.writeText(value); setCopied(true); }
    catch (cause) { setError(String(cause)); }
  }
  async function setAutoStartEnabled(enabled: boolean) {
    setError("");
    try { await invoke("autostart_set", { enabled }); setAutoStart(enabled); }
    catch (cause) { setError(String(cause)); }
  }
  async function setCloseToTrayEnabled(enabled: boolean) {
    setError("");
    try { await invoke("close_to_tray_set", { enabled }); setCloseToTray(enabled); }
    catch (cause) { setError(String(cause)); }
  }
  async function checkUpdate() {
    setUpdateBusy(true); setError("");
    try { setUpdateVersion(await invoke<string | null>("updater_check")); }
    catch (cause) { setError(String(cause)); }
    finally { setUpdateBusy(false); }
  }
  async function installUpdate() {
    if (!updateVersion) return;
    setUpdateBusy(true); setError("");
    try { await invoke("updater_install", { expectedVersion: updateVersion }); setUpdateVersion(null); }
    catch (cause) { setError(String(cause)); }
    finally { setUpdateBusy(false); }
  }
  function leaveFirstRun() { localStorage.setItem("localmux.seen", "yes"); setFirstRun(false); }

  return <main className="shell">
    <header className="topbar"><span className="brand">{config.productName}<small> / 智汇枢</small></span><span className="version">v{config.version}</span></header>
    <nav className="navigation" aria-label="Main"><button className={view === "home" ? "selected" : ""} onClick={() => setView("home")}>{t.home}</button><button className={view === "discover" ? "selected" : ""} onClick={() => setView("discover")}>{t.discover}</button><button className={view === "routing" ? "selected" : ""} onClick={() => setView("routing")}>{t.routing}</button><button className={view === "sessions" ? "selected" : ""} onClick={() => setView("sessions")}>{t.sessions}</button><button className={view === "api" ? "selected" : ""} onClick={() => setView("api")}>{t.api}</button></nav>
    {error && <p className="alert" role="alert">{error}</p>}
    {view === "home" && <><section className="hero"><p className="eyebrow">{t.phaseValue}</p><h1>{t.welcome}</h1><p>{t.intro}</p></section>
      {firstRun && <section className="notice"><strong>{t.firstRun}</strong><p>{t.steps}</p><button onClick={() => { setView("discover"); leaveFirstRun(); }}>{t.discover}</button><button className="quiet" onClick={leaveFirstRun}>{t.later}</button></section>}
      <section className="cards"><div className="card"><span>{t.detected}</span><strong>{report?.applications.length ?? "…"}</strong></div><div className="card"><span>{t.connected}</span><strong>{connectedApps.length}</strong></div><div className="card"><span>{t.apiTitle}</span><strong>{running ? t.running : t.stopped}</strong></div></section></>}
    {view === "discover" && <section className="page"><div className="section-head"><div><p className="eyebrow">{t.phaseValue}</p><h1>{t.detected}</h1></div><button onClick={rescan} disabled={scanning}>{scanning ? t.scanning : t.scan}</button></div>
      {!report && !error && <p aria-live="polite">{t.scanning}</p>}
      {report && <>{report.applications.length === 0 && <p className="empty">{t.none}</p>}
        <div className="app-list">{report.applications.map((app, index) => <article className="app-row" key={`${app.app_id}-${app.install_path}-${index}`}><div><h2>{app.name}</h2><p>{app.version ?? "—"} · {app.connectability === "BROKEN_INSTALL" ? t.broken : isConnectable(app) ? connectedApps.some(([id, path]) => id === app.app_id && path === app.executable) ? t.connectedState : t.versionUnverified : addedApps.some(([id, path]) => id === app.app_id && path === app.install_path) ? t.addedState : t.detectedOnly}</p><small>{app.install_path}</small><details><summary>{t.diagnostics}</summary><ul>{app.diagnostics.map(item => <li key={item}>{item}</li>)}</ul></details>{selectedModels[app.app_id]?.length > 0 && <p>{selectedModels[app.app_id].join(", ")}</p>}</div>{isConnectable(app)
          ? <button disabled={connecting !== null} onClick={() => connectedApps.some(([id, path]) => id === app.app_id && path === app.executable) ? disconnectApp(app.app_id) : connectApp(app.app_id, app.executable!)}>{connecting === app.app_id ? t.connecting : connectedApps.some(([id, path]) => id === app.app_id && path === app.executable) ? t.disconnect : t.add}</button>
          : isDesktopAddable(app) ? <button disabled={addingApp !== null} onClick={() => toggleDetectedApp(app.app_id, app.install_path)}>{addingApp === app.install_path ? t.connecting : addedApps.some(([id, path]) => id === app.app_id && path === app.install_path) ? t.removeDesktop : t.addDesktop}</button>
          : <button disabled title={t.pending}>{t.add}</button>}</article>)}</div>
        {report.applications.some(app => app.app_id === "mimo") && <div className="card mimo-connect"><h2>Xiaomi MiMo API</h2><p>{locale === "zh" ? "使用小米开放平台 API Key 连接；桌面应用登录态不与 API 共用。Key 仅存系统凭据库。" : "Connect with a MiMo API key. Desktop sign-in is separate. The key stays in the OS credential store."}</p>{connectedApps.some(([id]) => id === "mimo") ? <button disabled={connecting !== null} onClick={() => disconnectApp("mimo")}>{t.disconnect}</button> : <><label>API Key <input type="password" autoComplete="off" value={mimoKey} onChange={event => setMimoKey(event.target.value)} /></label><button disabled={connecting !== null || !mimoKey} onClick={connectMimo}>{connecting === "mimo" ? t.connecting : locale === "zh" ? "连接 MiMo API" : "Connect MiMo API"}</button></>}</div>}
        {report.applications.some(app => app.app_id === "qoder" && app.detection_source !== "executable") && !report.applications.some(app => app.app_id === "qoder" && isConnectable(app)) && <p className="notice">{locale === "zh" ? "Qoder 桌面版未提供可调用 CLI。请按官方说明安装并登录 Qoder CLI，然后重新扫描。" : "Install and sign in to the official Qoder CLI, then rescan."} <a href="https://docs.qoder.com/cli/installation" target="_blank" rel="noreferrer">{locale === "zh" ? "官方安装说明" : "Official installation guide"}</a></p>}
        <details className="roots"><summary>{t.roots} ({report.scanned_roots.length})</summary><ul>{report.scanned_roots.map(root => <li key={root}>{root}</li>)}</ul></details></>}
    </section>}
    {view === "routing" && <section className="page"><div className="section-head"><div><p className="eyebrow">{t.phaseValue}</p><h1>{t.routeTitle}</h1></div><button onClick={refreshRouting} disabled={routeBusy}>{t.refresh}</button></div>
      <div className="route-grid">{routeStatus.map(([alias, target]) => <div className="card" key={alias}><span>{alias}</span><strong>{target ?? t.unavailableRoute}</strong></div>)}</div>
      <section className="routing-form"><label><input type="checkbox" checked={policy.reserve_as_free} onChange={event => setPolicy(current => ({ ...current, reserve_as_free: event.target.checked }))} />{t.reserveFree}</label>
        <label><input type="checkbox" checked={policy.allow_metered} onChange={event => setPolicy(current => ({ ...current, allow_metered: event.target.checked }))} />{t.metered}</label>
        <label>{t.bestModel}<input value={policy.best_model ?? ""} placeholder="codex/model-id" onChange={event => setPolicy(current => ({ ...current, best_model: event.target.value }))} /></label>
        <label>{t.priority}<input value={policy.priority.join(", ")} onChange={event => setPolicy(current => ({ ...current, priority: event.target.value.split(",").map(item => item.trim()) }))} /></label>
        <button onClick={saveRouting} disabled={routeBusy}>{routeBusy ? t.saving : t.save}</button>
      </section>
      <h2>{t.quota}</h2>{quotaSnapshots.length === 0 ? <p>{t.quotaUnknown}</p> : <ul>{quotaSnapshots.map(item => <li key={item.connection_id}><strong>{item.connection_id}</strong>: {item.source === "PROVIDER_REPORTED" && item.remaining !== null ? item.unit?.startsWith("%") ? `${locale === "zh" ? "剩余" : "Remaining"} ${item.remaining}% ${item.unit.slice(1)}` : `${item.remaining} / ${item.limit ?? t.unknown} ${item.unit ?? ""}` : t.quotaUnknown} · {item.source === "PROVIDER_REPORTED" ? t.providerReported : item.source === "ESTIMATED" ? t.estimated : t.unknown}{item.reset_at && ` · ${locale === "zh" ? "重置" : "Resets"} ${new Date(item.reset_at * 1000).toLocaleString(locale === "zh" ? "zh-CN" : "en-US")}`}{item.observed_at > 0 && ` · ${new Date(item.observed_at * 1000).toLocaleString(locale === "zh" ? "zh-CN" : "en-US")}`}{quotaGuides[item.connection_id] && <> · <a href={quotaGuides[item.connection_id]} target="_blank" rel="noreferrer">{locale === "zh" ? "查看官方额度入口" : "Official quota guide"}</a></>}</li>)}</ul>}
    </section>}
    {view === "sessions" && <section className="page"><div className="section-head"><div><p className="eyebrow">{t.phaseValue}</p><h1>{t.sessionTitle}</h1></div><button onClick={refreshSessions}>{t.sessionRefresh}</button></div>
      {sessions.length === 0 ? <p className="empty">{t.sessionEmpty}</p> : <div className="session-list">{sessions.map(session => <article className="card" key={session.id}><strong>{session.model_id}</strong><span>{session.adapter_id} · {session.resume_supported ? t.sessionResume : t.sessionNoResume}</span><small>{session.id}</small><span>{t.lastActive}: {new Date(session.last_active_at * 1000).toLocaleString(locale === "zh" ? "zh-CN" : "en-US")}</span><button onClick={() => showSessionEvents(session.id)}>{t.sessionEvents}</button></article>)}</div>}
      {selectedSession && <section className="events"><h2>{t.sessionEvents}</h2>{sessionEvents.length === 0 ? <p>{t.noEvents}</p> : <ul>{sessionEvents.map((event, index) => <li key={`${event.kind}-${event.observed_at}-${index}`}>{event.kind} · {new Date(event.observed_at * 1000).toLocaleString(locale === "zh" ? "zh-CN" : "en-US")}<code>{event.detail_json}</code></li>)}</ul>}</section>}
    </section>}
    {view === "api" && <section className="page"><p className="eyebrow">{t.phaseValue}</p><h1>{t.apiTitle}</h1><div className="card api-card"><span>{running ? t.running : t.stopped}</span><code>{baseUrl || "…"}</code><button onClick={toggleGateway}>{running ? t.stop : t.start}</button></div>
      <div className="card api-card"><div className="routing-form key-policy"><label>{t.profile}<select value={keyPolicy.execution_profile} onChange={event => setKeyPolicy(current => ({ ...current, execution_profile: event.target.value as KeyPolicy["execution_profile"] }))}><option value="INFERENCE_ONLY">{t.inferenceOnly}</option><option value="READ_ONLY_AGENT">{t.readOnlyAgent}</option></select></label>{keyPolicy.execution_profile === "READ_ONLY_AGENT" && <p className="notice">{t.agentWarning}</p>}<label>{t.allowedRoutes}<input value={keyRoutesText} onChange={event => setKeyRoutesText(event.target.value)} /></label><label>{t.allowedConnections}<input value={keyConnectionsText} onChange={event => setKeyConnectionsText(event.target.value)} /></label><label>{t.rateLimit}<input type="number" min="1" max="600" value={keyPolicy.rate_limit_per_minute} onChange={event => setKeyPolicy(current => ({ ...current, rate_limit_per_minute: Number(event.target.value) }))} /></label><button onClick={createKey}>{t.createKey}</button></div>{key && <><p>{t.keyOnce}</p><div className="key-row"><code>{key.key}</code><button onClick={() => copyText(key.key)}>{copied ? t.copied : t.copy}</button></div></>}{keys.length > 0 && <div className="key-list"><h2>{locale === "zh" ? "已创建的 Key" : "Created keys"}</h2>{keys.map(item => <div className="key-entry" key={item.id}><div><code>{item.id}</code><p>{item.policy.execution_profile === "READ_ONLY_AGENT" ? t.readOnlyAgent : t.inferenceOnly} · {item.policy.rate_limit_per_minute}/min</p><small>{item.policy.allowed_routes.length ? item.policy.allowed_routes.join(", ") : locale === "zh" ? "全部路由" : "All routes"} · {item.policy.allowed_connections.length ? item.policy.allowed_connections.join(", ") : locale === "zh" ? "全部连接" : "All connections"}</small></div><button className="quiet" onClick={() => revokeKey(item.id)}>{t.revoke}</button></div>)}</div>}</div>
      <section className="routing-form"><h2>{locale === "zh" ? "桌面与更新" : "Desktop and updates"}</h2><label><input type="checkbox" checked={autoStart} onChange={event => setAutoStartEnabled(event.target.checked)} />{locale === "zh" ? "登录系统时启动智汇枢" : "Launch LocalMux at login"}</label><label><input type="checkbox" checked={closeToTray} onChange={event => setCloseToTrayEnabled(event.target.checked)} />{locale === "zh" ? "关闭窗口后继续在托盘运行" : "Keep running in tray when window closes"}</label><p>{updaterReady ? locale === "zh" ? "更新源已配置；安装更新需要再次点击。" : "Update source configured; installation needs a separate click." : locale === "zh" ? "尚未配置签名公钥和更新源，更新功能暂不可用。" : "Signing key and update source are not configured yet."}</p><button onClick={checkUpdate} disabled={!updaterReady || updateBusy}>{locale === "zh" ? "检查更新" : "Check for updates"}</button>{updateVersion !== null && <button onClick={installUpdate} disabled={updateBusy}>{locale === "zh" ? `安装 ${updateVersion}` : `Install ${updateVersion}`}</button>}{updaterReady && !updateBusy && updateVersion === null && <small>{locale === "zh" ? "检查后如无新版本，将保持当前版本。" : "If no update is found, the current version remains."}</small>}</section></section>}
    <footer><label>{t.language}<select value={locale} onChange={event => setLocale(event.target.value as Locale)}><option value="zh">中文</option><option value="en">English</option></select></label><label>{t.theme}<select value={theme} onChange={event => setTheme(event.target.value as Theme)}><option value="system">{t.system}</option><option value="light">{t.light}</option><option value="dark">{t.dark}</option></select></label></footer>
  </main>;
}
export default App;
