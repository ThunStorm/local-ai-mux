import CallRecords from "./CallRecords";
import ChatPlayground from "./ChatPlayground";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import config from "../src-tauri/tauri.conf.json";
import MacShell, { type Locale, type Theme, type View } from "./MacShell";
import Overview from "./Overview";
import ApiQuickSetup from "./ApiQuickSetup";
import ApplicationInventory, { type Detection, type QuotaBucket } from "./ApplicationInventory";
import "./App.css";

type ScanReport = { applications: Detection[]; scanned_roots: string[] };
type NewApiKey = { id: string; key: string };
type KeyPolicy = { allowed_routes: string[]; allowed_connections: string[]; execution_profile: "INFERENCE_ONLY" | "READ_ONLY_AGENT"; rate_limit_per_minute: number };
type KeySummary = { id: string; scopes: string[]; policy: KeyPolicy };
type RoutePolicy = { reserve_as_free: boolean; allow_metered: boolean; best_model: string | null; priority: string[] };
type QuotaSnapshot = { connection_id: string; remaining: number | null; limit: number | null; unit: string | null; reset_at: number | null; source: string; observed_at: number };
type Conversation = { id: string; model_id: string; adapter_id: string; provider_session_id: string | null; resume_supported: boolean; created_at: number; last_active_at: number; status: string };
type RunEvent = { kind: string; detail_json: string; observed_at: number };
type AuthStatus = "CHECKING" | "LOGIN_PENDING" | "READY" | "AUTH_REQUIRED" | "ERROR";
type RuntimeStatus = { state: "ACTIVE" | "IDLE" | "BUSY" | "COOLDOWN" | "UNAVAILABLE"; observed_at: number; source: string };
type ModelTestStatus = "VERIFYING" | "VERIFIED" | "FAILED";
const supportedAppIds = new Set(["codex", "qoder", "qoder-cn", "codebuddy", "mimo"]);
const defaultPolicy: RoutePolicy = { reserve_as_free: false, allow_metered: false, best_model: null, priority: [] };
const defaultKeyPolicy: KeyPolicy = { allowed_routes: [], allowed_connections: [], execution_profile: "INFERENCE_ONLY", rate_limit_per_minute: 60 };
const isConnectable = (app: Detection) => app.connectability === "SUPPORTED_VERSION_UNKNOWN" && !!app.executable;
const quotaGuides: Record<string, string> = { qoder: "https://docs.qoder.com/cli/usage", codebuddy: "https://www.codebuddy.cn/docs/workbuddy/Credits", mimo: "https://platform.xiaomimimo.com/" };
const copy = {
  zh: { home: "总览", discover: "应用与连接", routing: "路由", sessions: "会话", api: "API", welcome: "欢迎使用LocalAiMux", intro: "扫描已安装的 AI 应用，显式连接官方运行时，再复制本机 API 配置。", phase: "当前阶段", phaseValue: "本机桌面 AI API", detected: "发现应用", scan: "重新扫描", scanning: "扫描中…", none: "未发现目标应用", roots: "已检查的位置", diagnostics: "诊断信息", detectedOnly: "仅检测到，尚未连接", broken: "安装不完整", add: "添加", addDesktop: "添加应用", addedState: "已添加应用（仅登记）", removeDesktop: "移除应用", pending: "该 Adapter 尚未接入；后续阶段开放连接", firstRun: "首次使用", steps: "欢迎 → 扫描 → 连接 → 开启 API", later: "稍后继续", apiTitle: "本地 API", running: "运行中", stopped: "未启动", start: "启动 API", stop: "停止 API", createKey: "创建 API Key", keyOnce: "请立即复制。此 Key 仅显示一次，数据库只存哈希。", copy: "复制", copied: "已复制", revoke: "撤销此 Key", language: "语言", theme: "外观", system: "跟随系统", light: "浅色", dark: "深色", unavailable: "桌面服务不可用", connected: "已保存连接", disconnect: "断开", connecting: "连接中…", connectedState: "已连接", loginHint: "请先在对应官方 CLI 完成登录，再重试连接", versionUnverified: "CLI 版本未验证", routeTitle: "虚拟路由", refresh: "刷新目录与额度", unavailableRoute: "暂无符合条件的模型", reserveFree: "允许已确认可用的 Reserve 进入 free", metered: "balanced 允许计费模型（可能产生费用）", bestModel: "best 指定模型 ID", priority: "优先级模型 ID（逗号分隔）", save: "保存路由策略", saving: "保存中…", quota: "官方额度", quotaUnknown: "上游尚未报告额度，状态未知", providerReported: "上游报告", estimated: "估算", unknown: "未知", sessionTitle: "会话记录", sessionEmpty: "暂无续接会话。完整完成且允许保存的 Responses 调用会出现在这里。", sessionResume: "支持上游续接", sessionNoResume: "上游不支持续接", sessionEvents: "路由事件", noEvents: "暂无路由事件", lastActive: "最近活动", sessionRefresh: "刷新会话", profile: "执行权限", inferenceOnly: "仅推理（默认）", readOnlyAgent: "只读 Agent", agentWarning: "只读 Agent 允许调用已连接的官方 CLI。LocalAiMux 会请求只读模式，但上游运行时仍自行管理工具权限。", allowedRoutes: "允许的模型 / 虚拟路由（逗号分隔，留空为全部）", allowedConnections: "允许的连接 ID（逗号分隔，留空为全部）", rateLimit: "每分钟请求上限" },
  en: { home: "Overview", discover: "Apps & connections", routing: "Routing", sessions: "Sessions", api: "API", welcome: "Welcome to LocalAiMux", intro: "Find installed AI apps, explicitly connect official runtimes, then copy a local API configuration.", phase: "Current phase", phaseValue: "Desktop AI API", detected: "Found apps", scan: "Rescan", scanning: "Scanning…", none: "No target apps found", roots: "Locations checked", diagnostics: "Diagnostics", detectedOnly: "Detected, not connected", broken: "Incomplete installation", add: "Add", addDesktop: "Add app", addedState: "Added app (inventory only)", removeDesktop: "Remove app", pending: "This adapter is not available yet", firstRun: "First run", steps: "Welcome → Scan → Connect → Start API", later: "Continue later", apiTitle: "Local API", running: "Running", stopped: "Stopped", start: "Start API", stop: "Stop API", createKey: "Create API key", keyOnce: "Copy now. This key is shown only once; the database stores its hash.", copy: "Copy", copied: "Copied", revoke: "Revoke this key", language: "Language", theme: "Appearance", system: "System", light: "Light", dark: "Dark", unavailable: "Desktop service unavailable", connected: "Saved connections", disconnect: "Disconnect", connecting: "Connecting…", connectedState: "Connected", loginHint: "Sign in through the official CLI, then retry", versionUnverified: "CLI version unverified", routeTitle: "Virtual routes", refresh: "Refresh catalog and quota", unavailableRoute: "No eligible model", reserveFree: "Allow confirmed available Reserve in free", metered: "Allow metered models in balanced (may incur charges)", bestModel: "Explicit model ID for best", priority: "Priority model IDs (comma separated)", save: "Save route policy", saving: "Saving…", quota: "Official quota", quotaUnknown: "No upstream quota report; status unknown", providerReported: "Provider reported", estimated: "Estimated", unknown: "Unknown", sessionTitle: "Conversations", sessionEmpty: "No resume sessions yet. Stored, completed Responses calls appear here.", sessionResume: "Upstream resume available", sessionNoResume: "Upstream resume unavailable", sessionEvents: "Routing events", noEvents: "No routing events", lastActive: "Last active", sessionRefresh: "Refresh sessions", profile: "Execution profile", inferenceOnly: "Inference only (default)", readOnlyAgent: "Read-only agent", agentWarning: "Read-only agent calls connected official CLIs. LocalAiMux requests read-only mode; the upstream runtime controls its own tool permissions.", allowedRoutes: "Allowed model / virtual route IDs (comma separated; blank = all)", allowedConnections: "Allowed connection IDs (comma separated; blank = all)", rateLimit: "Requests per minute" },
};

function App() {
  const [locale, setLocale] = useState<Locale>(() => localStorage.getItem("localmux.locale") === "en" ? "en" : "zh");
  const [theme, setTheme] = useState<Theme>(() => { const value = localStorage.getItem("localmux.theme"); return value === "light" || value === "dark" ? value : "system"; });
  const [view, setView] = useState<View>("home");
  const [firstRun, setFirstRun] = useState(() => localStorage.getItem("localmux.seen") !== "yes");
  const [report, setReport] = useState<ScanReport | null>(null);
  const [hiddenAppIds, setHiddenAppIds] = useState<string[]>(() => {
    try {
      const saved = JSON.parse(localStorage.getItem("localmux.hiddenApps") ?? "[]");
      return Array.isArray(saved) ? saved.filter((id): id is string => typeof id === "string") : [];
    } catch { return []; }
  });
  const [scanning, setScanning] = useState(false);
  const [running, setRunning] = useState(false);
  const [baseUrl, setBaseUrl] = useState("");
  const [policy, setPolicy] = useState<RoutePolicy>(defaultPolicy);
  const [routeStatus, setRouteStatus] = useState<[string, string | null][]>([]);
  const [quotaSnapshots, setQuotaSnapshots] = useState<QuotaSnapshot[]>([]);
  const [quotaBuckets, setQuotaBuckets] = useState<QuotaBucket[]>([]);
  const [routeBusy, setRouteBusy] = useState(false);
  const [sessions, setSessions] = useState<Conversation[]>([]);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [sessionEvents, setSessionEvents] = useState<RunEvent[]>([]);
  const [connectedApps, setConnectedApps] = useState<[string, string][]>([]);
  const [addedApps, setAddedApps] = useState<[string, string][]>([]);
  const [addingApp, setAddingApp] = useState<string | null>(null);
  const [runtimeStatuses, setRuntimeStatuses] = useState<Record<string, RuntimeStatus>>({});
  const [selectedModels, setSelectedModels] = useState<Record<string, string[]>>({});
  const [authStatuses, setAuthStatuses] = useState<Record<string, AuthStatus>>({});
  const [modelTests, setModelTests] = useState<Record<string, ModelTestStatus>>({});
  const loginAttempts = useRef<Record<string, number>>({});
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
  const [error, setErrorState] = useState<{ view: View; message: string } | null>(null);
  const [authMessage, setAuthMessage] = useState("");
  const setError = (message: string) => setErrorState(message ? { view, message } : null);
  const t = copy[locale];
  const supportedApplications = report?.applications.filter(app => supportedAppIds.has(app.app_id)) ?? [];
  const shownApplications = supportedApplications.filter(app => !hiddenAppIds.includes(app.app_id));
  const hiddenApplications = [...new Map(supportedApplications.filter(app => hiddenAppIds.includes(app.app_id)).map(app => [app.app_id, app.name])).entries()];

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
    invoke<QuotaBucket[]>("quota_buckets").then(value => { if (active) setQuotaBuckets(value); }).catch(() => {});
    const loadRuntime = () => invoke<Record<string, RuntimeStatus>>("connection_runtime_status").then(value => { if (active) setRuntimeStatuses(value); }).catch(() => { if (active) setRuntimeStatuses({}); });
    void loadRuntime();
    const quotaTimer = window.setInterval(() => {
      void loadRuntime();
      invoke<QuotaBucket[]>("quota_buckets").then(value => { if (active) setQuotaBuckets(value); }).catch(() => {});
    }, 10_000);
    invoke<Conversation[]>("sessions_list").then(value => { if (active) setSessions(value); }).catch(() => {});
    invoke<boolean>("autostart_status").then(value => { if (active) setAutoStart(value); }).catch(() => {});
    invoke<boolean>("close_to_tray_status").then(value => { if (active) setCloseToTray(value); }).catch(() => {});
    invoke<boolean>("updater_available").then(value => { if (active) setUpdaterReady(value); }).catch(() => {});
    invoke<KeySummary[]>("list_api_keys").then(value => { if (active) setKeys(value); }).catch(() => {});
    return () => { active = false; window.clearInterval(quotaTimer); };
  }, []);

  useEffect(() => {
    if (view !== "discover" || !report) return;
    let active = true;
    const checked = new Set<string>();
    for (const app of report.applications) {
      if ((app.app_id !== "codex" && app.app_id !== "qoder" && app.app_id !== "qoder-cn") || !isConnectable(app) || !app.executable || checked.has(app.app_id)) continue;
      checked.add(app.app_id);
      const path = app.executable;
      setAuthStatuses(current => ({ ...current, [path]: "CHECKING" }));
      invoke<AuthStatus>("cli_auth_status", { appId: app.app_id, executable: path })
        .then(status => { if (active) setAuthStatuses(current => ({ ...current, [path]: status })); })
        .catch(() => { if (active) setAuthStatuses(current => ({ ...current, [path]: "ERROR" })); });
    }
    return () => { active = false; };
  }, [view, report]);

  async function rescan() {
    setScanning(true); setError("");
    try { setReport(await invoke<ScanReport>("scan_apps")); }
    catch (cause) { setError(String(cause)); }
    finally { setScanning(false); }
  }
  function hideApp(appId: string) {
    setHiddenAppIds(current => {
      const next = [...new Set([...current, appId])];
      localStorage.setItem("localmux.hiddenApps", JSON.stringify(next));
      return next;
    });
  }
  function restoreApp(appId: string) {
    setHiddenAppIds(current => {
      const next = current.filter(id => id !== appId);
      localStorage.setItem("localmux.hiddenApps", JSON.stringify(next));
      return next;
    });
  }
  async function checkCliAuth(appId: string, executable: string) {
    setAuthStatuses(current => ({ ...current, [executable]: "CHECKING" }));
    try {
      const status = await invoke<AuthStatus>("cli_auth_status", { appId, executable });
      setAuthStatuses(current => ({ ...current, [executable]: status }));
    } catch (cause) {
      setAuthStatuses(current => ({ ...current, [executable]: "ERROR" }));
      setError(String(cause));
    }
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
      setAuthStatuses(current => ({ ...current, [executable]: "READY" }));
      setModelTests(current => Object.fromEntries(Object.entries(current).filter(([model]) => !model.startsWith(`${appId}/`))));
      setConnectedApps(current => [...current.filter(([id]) => id !== appId), [appId, executable]]);
      setRouteStatus(await invoke<[string, string | null][]>("routing_status"));
      setQuotaSnapshots(await invoke<QuotaSnapshot[]>("quota_snapshots"));
      setQuotaBuckets(await invoke<QuotaBucket[]>("quota_buckets"));
    } catch (cause) {
      const message = String(cause);
      if (message.includes("authentication required")) setAuthStatuses(current => ({ ...current, [executable]: "AUTH_REQUIRED" }));
      setError(locale === "zh" && message.includes("authentication required") ? appId === "qoder" || appId === "qoder-cn" ? `${appId === "qoder-cn" ? "Qoder CN" : "Qoder"} CLI 尚未登录；请在应用卡片启动官方登录，完成后验证连接。` : t.loginHint : locale === "zh" && appId === "codebuddy" && message.includes("model unavailable") ? "CodeBuddy CLI 尚未选定模型；请在官方 CLI 选择模型后重试。" : locale === "zh" && message.includes("adapter unsupported") ? `${appId} 官方 CLI 返回了当前版本无法识别的模型目录，请更新 CLI 后重试。` : message);
    } finally { setConnecting(null); }
  }
  async function loginCli(appId: string, executable: string) {
    setConnecting(`${appId}-login`); setError(""); setAuthMessage("");
    try {
      await invoke("cli_login", { appId, executable });
      setAuthStatuses(current => ({ ...current, [executable]: "LOGIN_PENDING" }));
      setAuthMessage(locale === "zh" ? "已启动官方登录，完成浏览器授权后会自动确认状态。" : "Official sign-in started. Status will be checked automatically after browser authorization.");
      const attempt = (loginAttempts.current[executable] ?? 0) + 1;
      loginAttempts.current[executable] = attempt;
      const poll = async (remaining: number) => {
        if (loginAttempts.current[executable] !== attempt) return;
        try {
          const status = await invoke<AuthStatus>("cli_auth_status", { appId, executable });
          if (status === "READY") {
            setAuthStatuses(current => ({ ...current, [executable]: "READY" }));
            setAuthMessage(locale === "zh" ? "官方登录已自动确认；请点击「验证连接」读取模型目录。" : "Sign-in confirmed. Select Verify connection to read the model catalog.");
            return;
          }
        } catch { /* Retry while the vendor login flow is still pending. */ }
        if (remaining > 0) window.setTimeout(() => { void poll(remaining - 1); }, 10_000);
        else {
          setAuthStatuses(current => ({ ...current, [executable]: "AUTH_REQUIRED" }));
          setAuthMessage(locale === "zh" ? "尚未确认登录；请完成官方授权后点击「检查登录」。" : "Sign-in is not confirmed. Finish authorization, then check sign-in again.");
        }
      };
      window.setTimeout(() => { void poll(18); }, 3_000);
    } catch (cause) { setError(String(cause)); }
    finally { setConnecting(null); }
  }
  async function verifyAppModel(model: string) {
    setModelTests(current => ({ ...current, [model]: "VERIFYING" })); setError("");
    try {
      await invoke<number>("verify_model", { model });
      setModelTests(current => ({ ...current, [model]: "VERIFIED" }));
    } catch (cause) {
      setModelTests(current => ({ ...current, [model]: "FAILED" }));
      if (String(cause).includes("authentication required")) {
        const provider = model.startsWith("mimocode/") ? "mimocode" : model.split("/")[0];
        const executable = connectedApps.find(([id]) => id === provider)?.[1];
        if (executable && !executable.includes("credential:")) setAuthStatuses(current => ({ ...current, [executable]: "AUTH_REQUIRED" }));
      }
      setError(String(cause));
    }
  }
  async function connectMimo() {
    setConnecting("mimo"); setError("");
    try {
      const models = await invoke<string[]>("connect_mimo", { apiKey: mimoKey });
      setMimoKey("");
      setSelectedModels(current => ({ ...current, mimo: models }));
      setConnectedApps(current => [...current.filter(([id]) => id !== "mimo"), ["mimo", "session-credential:mimo"]]);
      setRouteStatus(await invoke<[string, string | null][]>("routing_status"));
      setQuotaSnapshots(await invoke<QuotaSnapshot[]>("quota_snapshots"));
      setQuotaBuckets(await invoke<QuotaBucket[]>("quota_buckets"));
    } catch (cause) { setError(String(cause)); }
    finally { setConnecting(null); }
  }
  async function connectMimoCode(executable: string) {
    setConnecting("mimocode"); setError("");
    try {
      const models = await invoke<string[]>("connect_mimocode", { executable });
      setSelectedModels(current => ({ ...current, mimo: models }));
      setModelTests(current => Object.fromEntries(Object.entries(current).filter(([model]) => !model.startsWith("mimocode/"))));
      setConnectedApps(current => [...current.filter(([id]) => id !== "mimocode"), ["mimocode", executable]]);
      setRouteStatus(await invoke<[string, string | null][]>("routing_status"));
    } catch (cause) {
      if (String(cause).includes("authentication required")) setAuthStatuses(current => ({ ...current, [executable]: "AUTH_REQUIRED" }));
      setError(String(cause));
    }
    finally { setConnecting(null); }
  }
  async function disconnectApp(appId: string) {
    setConnecting(appId); setError("");
    try {
      await invoke("disconnect_app", { appId });
      setConnectedApps(current => current.filter(([id]) => id !== appId));
      setSelectedModels(current => { const next = { ...current }; delete next[appId === "mimocode" ? "mimo" : appId]; return next; });
      setRouteStatus(await invoke<[string, string | null][]>("routing_status"));
      setQuotaSnapshots(await invoke<QuotaSnapshot[]>("quota_snapshots"));
      setQuotaBuckets(await invoke<QuotaBucket[]>("quota_buckets"));
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
      setQuotaBuckets(await invoke<QuotaBucket[]>("quota_buckets"));
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

  return <MacShell view={view} onView={setView} locale={locale} onLocale={setLocale} theme={theme} onTheme={setTheme} version={config.version}>
    {error?.view === view && <p className="alert" role="alert">{error.message}</p>}
    {view === "home" && <Overview locale={locale} applications={shownApplications} connected={connectedApps} quotaBuckets={quotaBuckets} running={running} firstRun={firstRun} onLeaveFirstRun={leaveFirstRun} onView={setView} />}
    {view === "discover" && <section className="page"><header className="page-heading"><div><h1>{t.discover}</h1><p>{locale === "zh" ? `发现 ${report ? new Set(shownApplications.map(app => app.app_id)).size : "…"} 个应用 · 已保存 ${connectedApps.length} 个连接` : `${report ? new Set(shownApplications.map(app => app.app_id)).size : "…"} apps found · ${connectedApps.length} saved connections`}</p></div><button onClick={rescan} disabled={scanning}>{scanning ? t.scanning : t.scan}</button></header>
      {!report && error?.view !== view && <p aria-live="polite">{t.scanning}</p>}
      {report && <>{shownApplications.length === 0 && <p className="empty">{t.none}</p>}
        {authMessage && <p className="notice" role="status">{authMessage}</p>}
        <ApplicationInventory runtimeStatuses={runtimeStatuses} applications={shownApplications} connected={connectedApps} added={addedApps} selectedModels={selectedModels} authStatuses={authStatuses} modelTests={modelTests} quotaBuckets={quotaBuckets} connecting={connecting} adding={addingApp} locale={locale} onConnect={connectApp} onConnectMimoCode={connectMimoCode} onCheckAuth={checkCliAuth} onCliLogin={loginCli} onVerifyModel={verifyAppModel} onHide={hideApp} onDisconnect={disconnectApp} onToggleAdded={toggleDetectedApp} />
        {hiddenApplications.length > 0 && <details className="roots"><summary>{locale === "zh" ? `已移出列表 (${hiddenApplications.length})` : `Hidden apps (${hiddenApplications.length})`}</summary><ul>{hiddenApplications.map(([id, name]) => <li key={id}>{name} <button className="quiet" onClick={() => restoreApp(id)}>{locale === "zh" ? "恢复显示" : "Restore"}</button></li>)}</ul></details>}
        {shownApplications.some(app => app.app_id === "mimo") && <div className="card mimo-connect"><h2>Xiaomi MiMo API</h2><p>{locale === "zh" ? "使用小米开放平台 API Key 连接；桌面应用登录态不与 API 共用。研发版仅在本次运行的内存中保存 Key，重启后需要重新连接。" : "Connect with a MiMo API key. Desktop sign-in is separate. This development build keeps the key in memory only; reconnect after restart."}</p>{connectedApps.some(([id]) => id === "mimo") ? <button disabled={connecting !== null} onClick={() => disconnectApp("mimo")}>{t.disconnect}</button> : <><label>API Key <input type="password" autoComplete="off" value={mimoKey} onChange={event => setMimoKey(event.target.value)} /></label><button disabled={connecting !== null || !mimoKey} onClick={connectMimo}>{connecting === "mimo" ? t.connecting : locale === "zh" ? "连接 MiMo API" : "Connect MiMo API"}</button></>}</div>}
        {report.applications.some(app => app.app_id === "qoder" && app.detection_source !== "executable") && !report.applications.some(app => app.app_id === "qoder" && isConnectable(app)) && <p className="notice">{locale === "zh" ? "Qoder 桌面版未提供可调用 CLI。请按官方说明安装并登录 Qoder CLI，然后重新扫描。" : "Install and sign in to the official Qoder CLI, then rescan."} <a href="https://docs.qoder.com/cli/installation" target="_blank" rel="noreferrer">{locale === "zh" ? "官方安装说明" : "Official installation guide"}</a></p>}
        <details className="roots"><summary>{t.roots} ({report.scanned_roots.length})</summary><ul>{report.scanned_roots.map(root => <li key={root}>{root}</li>)}</ul></details></>}
    </section>}
    {view === "routing" && <section className="page"><header className="page-heading"><div><h1>{t.routeTitle}</h1><p>{locale === "zh" ? "旧配置兼容" : "Legacy configuration"}</p></div><button onClick={refreshRouting} disabled={routeBusy}>{t.refresh}</button></header>
      <div className="route-grid">{routeStatus.map(([alias, target]) => <div className="card" key={alias}><span>{alias}</span><strong>{target ?? t.unavailableRoute}</strong></div>)}</div>
      <section className="routing-form"><label><input type="checkbox" checked={policy.reserve_as_free} onChange={event => setPolicy(current => ({ ...current, reserve_as_free: event.target.checked }))} />{t.reserveFree}</label>
        <label><input type="checkbox" checked={policy.allow_metered} onChange={event => setPolicy(current => ({ ...current, allow_metered: event.target.checked }))} />{t.metered}</label>
        <label>{t.bestModel}<input value={policy.best_model ?? ""} placeholder="codex/model-id" onChange={event => setPolicy(current => ({ ...current, best_model: event.target.value }))} /></label>
        <label>{t.priority}<input value={policy.priority.join(", ")} onChange={event => setPolicy(current => ({ ...current, priority: event.target.value.split(",").map(item => item.trim()) }))} /></label>
        <button onClick={saveRouting} disabled={routeBusy}>{routeBusy ? t.saving : t.save}</button>
      </section>
      <h2>{t.quota}</h2>{quotaSnapshots.length === 0 ? <p>{t.quotaUnknown}</p> : <ul>{quotaSnapshots.map(item => <li key={item.connection_id}><strong>{item.connection_id}</strong>: {item.source === "PROVIDER_REPORTED" && item.remaining !== null ? item.unit?.startsWith("%") ? `${locale === "zh" ? "剩余" : "Remaining"} ${item.remaining}% ${item.unit.slice(1)}` : `${item.remaining} / ${item.limit ?? t.unknown} ${item.unit ?? ""}` : t.quotaUnknown} · {item.source === "PROVIDER_REPORTED" ? t.providerReported : item.source === "ESTIMATED" ? t.estimated : t.unknown}{item.reset_at && ` · ${locale === "zh" ? "重置" : "Resets"} ${new Date(item.reset_at * 1000).toLocaleString(locale === "zh" ? "zh-CN" : "en-US")}`}{item.observed_at > 0 && ` · ${new Date(item.observed_at * 1000).toLocaleString(locale === "zh" ? "zh-CN" : "en-US")}`}{quotaGuides[item.connection_id] && <> · <a href={quotaGuides[item.connection_id]} target="_blank" rel="noreferrer">{locale === "zh" ? "查看官方额度入口" : "Official quota guide"}</a></>}</li>)}</ul>}
    </section>}
    {view === "sessions" && <section className="page"><header className="page-heading"><div><h1>{t.sessionTitle}</h1></div></header>
      <CallRecords locale={locale} />
      <details className="resume-sessions"><summary>{locale === "zh" ? "Responses 续接会话与路由事件" : "Responses sessions and routing events"}</summary>
      <button className="resume-refresh" onClick={refreshSessions}>{t.sessionRefresh}</button>
      {sessions.length === 0 ? <p className="empty">{t.sessionEmpty}</p> : <div className="session-list">{sessions.map(session => <article className="card" key={session.id}><strong>{session.model_id}</strong><span>{session.adapter_id} · {session.resume_supported ? t.sessionResume : t.sessionNoResume}</span><small>{session.id}</small><span>{t.lastActive}: {new Date(session.last_active_at * 1000).toLocaleString(locale === "zh" ? "zh-CN" : "en-US")}</span><button onClick={() => showSessionEvents(session.id)}>{t.sessionEvents}</button></article>)}</div>}
      {selectedSession && <section className="events"><h2>{t.sessionEvents}</h2>{sessionEvents.length === 0 ? <p>{t.noEvents}</p> : <ul>{sessionEvents.map((event, index) => <li key={`${event.kind}-${event.observed_at}-${index}`}>{event.kind} · {new Date(event.observed_at * 1000).toLocaleString(locale === "zh" ? "zh-CN" : "en-US")}<code>{event.detail_json}</code></li>)}</ul>}</section>}
      </details>
    </section>}
    {view === "api" && <section className="page api-page"><header className="page-heading"><div><h1>{t.apiTitle}</h1><p className="service-state"><span className={running ? "status-dot live" : "status-dot"} />{running ? t.running : t.stopped} · {locale === "zh" ? "仅本机访问" : "Loopback only"}</p></div><button className="primary" onClick={toggleGateway}>{running ? t.stop : t.start}</button></header>
      <ApiQuickSetup running={running} baseUrl={baseUrl} connections={connectedApps.map(([id, path]) => `${id}:${path}`).join("|")} locale={locale} />
      <details className="advanced-key"><summary>{locale === "zh" ? "高级 Key 管理" : "Advanced key management"}</summary><div className="card api-card"><div className="routing-form key-policy"><label>{t.profile}<select value={keyPolicy.execution_profile} onChange={event => setKeyPolicy(current => ({ ...current, execution_profile: event.target.value as KeyPolicy["execution_profile"] }))}><option value="INFERENCE_ONLY">{t.inferenceOnly}</option><option value="READ_ONLY_AGENT">{t.readOnlyAgent}</option></select></label>{keyPolicy.execution_profile === "READ_ONLY_AGENT" && <p className="notice">{t.agentWarning}</p>}<label>{t.allowedRoutes}<input value={keyRoutesText} onChange={event => setKeyRoutesText(event.target.value)} /></label><label>{t.allowedConnections}<input value={keyConnectionsText} onChange={event => setKeyConnectionsText(event.target.value)} /></label><label>{t.rateLimit}<input type="number" min="1" max="600" value={keyPolicy.rate_limit_per_minute} onChange={event => setKeyPolicy(current => ({ ...current, rate_limit_per_minute: Number(event.target.value) }))} /></label><button onClick={createKey}>{t.createKey}</button></div>{key && <><p>{t.keyOnce}</p><div className="key-row"><code>{key.key}</code><button onClick={() => copyText(key.key)}>{copied ? t.copied : t.copy}</button></div></>}{keys.length > 0 && <div className="key-list"><h2>{locale === "zh" ? "已创建的 Key" : "Created keys"}</h2>{keys.map(item => <div className="key-entry" key={item.id}><div><code>{item.id}</code><p>{item.policy.execution_profile === "READ_ONLY_AGENT" ? t.readOnlyAgent : t.inferenceOnly} · {item.policy.rate_limit_per_minute}/min</p><small>{item.policy.allowed_routes.length ? item.policy.allowed_routes.join(", ") : locale === "zh" ? "全部路由" : "All routes"} · {item.policy.allowed_connections.length ? item.policy.allowed_connections.join(", ") : locale === "zh" ? "全部连接" : "All connections"}</small></div><button className="quiet" onClick={() => revokeKey(item.id)}>{t.revoke}</button></div>)}</div>}</div></details>
      <details className="inline-disclosure desktop-settings"><summary>{locale === "zh" ? "桌面与更新" : "Desktop and updates"}</summary><section className="routing-form"><label><input type="checkbox" checked={autoStart} onChange={event => setAutoStartEnabled(event.target.checked)} />{locale === "zh" ? "登录系统时启动LocalAiMux" : "Launch LocalAiMux at login"}</label><label><input type="checkbox" checked={closeToTray} onChange={event => setCloseToTrayEnabled(event.target.checked)} />{locale === "zh" ? "关闭窗口后继续在托盘运行" : "Keep running in tray when window closes"}</label><p>{updaterReady ? locale === "zh" ? "更新源已配置；安装更新需要再次点击。" : "Update source configured; installation needs a separate click." : locale === "zh" ? "尚未配置签名公钥和更新源，更新功能暂不可用。" : "Signing key and update source are not configured yet."}</p><button onClick={checkUpdate} disabled={!updaterReady || updateBusy}>{locale === "zh" ? "检查更新" : "Check for updates"}</button>{updateVersion !== null && <button onClick={installUpdate} disabled={updateBusy}>{locale === "zh" ? `安装 ${updateVersion}` : `Install ${updateVersion}`}</button>}{updaterReady && !updateBusy && updateVersion === null && <small>{locale === "zh" ? "检查后如无新版本，将保持当前版本。" : "If no update is found, the current version remains."}</small>}</section></details></section>}
    <ChatPlayground visible={view === "chat"} running={running} connections={connectedApps.map(([id]) => id).join("|")} locale={locale} onApi={() => setView("api")} />
  </MacShell>;
}
export default App;
