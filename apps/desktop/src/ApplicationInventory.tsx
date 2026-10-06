import { useState } from "react";
import SoftwareIcon from "./SoftwareIcon";

export type Detection = { app_id: string; name: string; version: string | null; executable: string | null; install_path: string; detection_source: string; connectability: string; diagnostics: string[] };
export type QuotaBucket = { connection_id: string; bucket_id: string; label: string; remaining: number | null; limit: number | null; unit: string | null; reset_at: number | null; source: string; observed_at: number };
export type AuthStatus = "CHECKING" | "LOGIN_PENDING" | "READY" | "AUTH_REQUIRED" | "ERROR";
export type ModelTestStatus = "VERIFYING" | "VERIFIED" | "FAILED";
type Filter = "all" | "saved" | "connectable" | "detected";
type Props = {
  applications: Detection[];
  connected: [string, string][];
  added: [string, string][];
  selectedModels: Record<string, string[]>;
  authStatuses: Record<string, AuthStatus>;
  modelTests: Record<string, ModelTestStatus>;
  quotaBuckets: QuotaBucket[];
  connecting: string | null;
  adding: string | null;
  locale: "zh" | "en";
  onConnect: (id: string, executable: string) => void;
  onConnectMimoCode: (executable: string) => void;
  onCheckAuth: (id: string, executable: string) => void;
  onCliLogin: (id: string, executable: string) => void;
  onVerifyModel: (model: string) => void;
  onHide: (id: string) => void;
  onDisconnect: (id: string) => void;
  onToggleAdded: (id: string, path: string) => void;
};

const connectable = (app: Detection) => app.connectability === "SUPPORTED_VERSION_UNKNOWN" && !!app.executable;
const addable = (app: Detection) => app.detection_source !== "executable" && app.detection_source !== "windows_app_paths" && app.connectability !== "BROKEN_INSTALL";

export default function ApplicationInventory({ applications, connected, added, selectedModels, authStatuses, modelTests, quotaBuckets, connecting, adding, locale, onConnect, onConnectMimoCode, onCheckAuth, onCliLogin, onVerifyModel, onHide, onDisconnect, onToggleAdded }: Props) {
  const zh = locale === "zh";
  const [preferred, setPreferred] = useState<Record<string, string>>({});
  const [modelToTest, setModelToTest] = useState<Record<string, string>>({});
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const groups = new Map<string, Detection[]>();
  for (const item of applications) groups.set(item.app_id, [...(groups.get(item.app_id) ?? []), item]);
  const saved = (id: string) => connected.find(([name, path]) => name === (id === "mimo" ? "mimocode" : id) && !path.includes("credential:"))?.[1];
  const category = (id: string, entries: Detection[]): Filter => saved(id) ? "saved" : entries.some(connectable) ? "connectable" : "detected";
  const all = [...groups];
  const visible = all.filter(([id, entries]) => (filter === "all" || category(id, entries) === filter) && entries.some(item => `${item.name} ${item.install_path} ${item.executable ?? ""}`.toLocaleLowerCase().includes(query.toLocaleLowerCase())));
  const filters: { id: Filter; label: string }[] = [
    { id: "all", label: zh ? "全部" : "All" },
    { id: "saved", label: zh ? "已保存" : "Saved" },
    { id: "connectable", label: zh ? "可添加" : "Connectable" },
    { id: "detected", label: zh ? "仅检测" : "Detected" },
  ];

  return <section className="inventory">
    <div className="inventory-tools"><input type="search" aria-label={zh ? "搜索应用或安装路径" : "Search app or path"} placeholder={zh ? "搜索应用或安装路径" : "Search app or installation path"} value={query} onChange={event => setQuery(event.target.value)} /><div className="segmented" aria-label={zh ? "应用状态筛选" : "App status filter"}>{filters.map(item => <button key={item.id} type="button" className={filter === item.id ? "active" : ""} aria-pressed={filter === item.id} onClick={() => setFilter(item.id)}>{item.label}<span>{item.id === "all" ? all.length : all.filter(([id, entries]) => category(id, entries) === item.id).length}</span></button>)}</div></div>
    <div className="inventory-list" role="list">{visible.length === 0 && <p className="empty-state">{zh ? "没有符合条件的应用。" : "No apps match this view."}</p>}{visible.map(([id, entries]) => {
      const desktop = entries.find(addable);
      const bridges = entries.filter(connectable);
      const bundled = desktop && bridges.find(item => item.install_path === desktop.install_path);
      const choice = bridges.find(item => item.executable === preferred[id]) ?? bundled ?? bridges[0];
      const connectedPath = saved(id);
      const isAdded = desktop && added.some(([name, path]) => name === id && path === desktop.install_path);
      const display = desktop ?? choice ?? entries[0];
      const modelCount = selectedModels[id]?.length;
      const selectedModel = selectedModels[id]?.includes(modelToTest[id]) ? modelToTest[id] : selectedModels[id]?.[0];
      const auth = choice ? authStatuses[choice.executable!] : undefined;
      const authLabel = auth === "READY" ? zh ? "已登录" : "Signed in" : auth === "AUTH_REQUIRED" ? zh ? "需要登录" : "Sign-in required" : auth === "CHECKING" || auth === "LOGIN_PENDING" ? zh ? "登录状态确认中" : "Checking sign-in" : auth === "ERROR" ? zh ? "登录状态无法确认" : "Sign-in status unavailable" : null;
      const authApplies = id === "codex" || id === "qoder" || id === "qoder-cn" || auth === "AUTH_REQUIRED";
      const buckets = id === "mimo" ? [] : quotaBuckets.filter(item => item.connection_id === id);
      const status = connectedPath ? id === "mimo" ? zh ? "MiMo Code 本次运行已连接 · 令牌30分钟" : "MiMo Code connected · 30-minute token" : zh ? "已保存连接，运行状态待检" : "Saved connection; runtime unchecked" : choice ? zh ? "官方运行时待验证" : "Runtime not yet verified" : isAdded ? zh ? "已登记应用，暂无 API 桥" : "App recorded; no API bridge" : desktop ? zh ? "仅检测到，暂无 API 桥" : "Detected; no API bridge" : zh ? "暂不支持 API 接入" : "API connection unsupported";
      return <article className="inventory-row" role="listitem" key={id}>
        <div className="inventory-main"><SoftwareIcon id={id} name={display.name} /><div className="inventory-identity"><h2>{display.name}</h2><p>{display.version ? `${zh ? "版本" : "Version"} ${display.version}` : zh ? "版本未知" : "Version unknown"}{connectedPath && modelCount !== undefined && ` · ${modelCount} ${zh ? "个已报告模型" : "reported models"}`}</p><small title={display.install_path}>{display.install_path}</small></div></div>
        <div className="inventory-state"><span className={`state-label ${connectedPath ? "saved" : ""}`}><span className="status-dot" />{status}</span>{choice && authApplies && <small className="auth-state">{authLabel ?? (zh ? "登录状态待检查" : "Sign-in unchecked")}</small>}{connectedPath && <div className="quota-lines">{buckets.length === 0 ? <small>{zh ? "额度未知" : "Quota unknown"}</small> : buckets.map(bucket => {
          const fresh = bucket.observed_at > 0 && Date.now() / 1000 - bucket.observed_at <= 600;
          const value = bucket.source === "PROVIDER_REPORTED" && bucket.remaining !== null ? `${bucket.remaining}${bucket.unit === "%" ? "%" : ` ${bucket.unit ?? ""}`}` : zh ? "未知" : "Unknown";
          const label = bucket.bucket_id === "legacy" ? zh ? "上游额度" : "Upstream quota" : zh ? bucket.label === "primary" ? "主窗口" : bucket.label === "secondary" ? "次窗口" : bucket.label.replace(" min window", " 分钟窗口") : bucket.label;
          return <small key={bucket.bucket_id}>{label} · {value}{!fresh && (zh ? " · 已过期" : " · Stale")}</small>;
        })}</div>}</div>
        <div className="inventory-action">
          {choice && auth === "AUTH_REQUIRED" && (id === "codebuddy" ? <a href="https://www.codebuddy.cn/docs/cli/quickstart" target="_blank" rel="noreferrer">{zh ? "登录说明" : "Sign-in guide"}</a> : <button disabled={connecting !== null} onClick={() => onCliLogin(id, choice.executable!)}>{zh ? `登录 ${id === "mimo" ? "MiMo Code" : display.name}` : `Sign in to ${id === "mimo" ? "MiMo Code" : display.name}`}</button>)}
          {choice && (id === "codex" || id === "qoder" || id === "qoder-cn") && (!auth || auth === "ERROR") && <button disabled={connecting !== null} onClick={() => onCheckAuth(id, choice.executable!)}>{zh ? "检查登录" : "Check sign-in"}</button>}
          {connectedPath
            ? <>{id !== "mimo" && <button disabled={connecting !== null || !choice || auth === "AUTH_REQUIRED" || auth === "CHECKING" || auth === "LOGIN_PENDING"} onClick={() => choice && onConnect(id, choice.executable!)}>{connecting === id ? zh ? "验证中…" : "Verifying…" : zh ? "验证连接" : "Verify connection"}</button>}<button disabled={connecting !== null} onClick={() => onDisconnect(id === "mimo" ? "mimocode" : id)}>{zh ? "断开" : "Disconnect"}</button></>
            : choice ? <button className="primary" disabled={connecting !== null || auth === "AUTH_REQUIRED" || auth === "CHECKING" || auth === "LOGIN_PENDING"} onClick={() => id === "mimo" ? onConnectMimoCode(choice.executable!) : onConnect(id, choice.executable!)}>{connecting === id || connecting === "mimocode" ? zh ? "验证中…" : "Verifying…" : zh ? "验证连接" : "Verify connection"}</button>
            : desktop ? <button disabled={adding !== null} onClick={() => onToggleAdded(id, desktop.install_path)}>{adding === desktop.install_path ? zh ? "处理中…" : "Working…" : isAdded ? zh ? "移除登记" : "Remove record" : zh ? "登记应用" : "Record app"}</button>
            : <span className="unavailable-action">{zh ? "暂不支持" : "Unsupported"}</span>}

        </div>
        <details className="inventory-detail"><summary>{zh ? "安装与诊断" : "Installations and diagnostics"}</summary><div className="inventory-secondary-actions"><button className="quiet" title={zh ? "仅从列表隐藏，可在下方恢复" : "Hide from list; restore below"} onClick={() => onHide(id)}>{zh ? "移出列表" : "Hide"}</button></div>{bridges.length > 1 && !connectedPath && <label>{zh ? "接入方式" : "Runtime"}<select value={choice?.executable ?? ""} onChange={event => setPreferred(current => ({ ...current, [id]: event.target.value }))}>{bridges.map(item => <option key={item.executable} value={item.executable!}>{item.executable}</option>)}</select></label>}<ul>{entries.map(item => <li key={`${item.detection_source}:${item.install_path}:${item.executable ?? ""}`}><code>{item.executable ?? item.install_path}</code>{item.diagnostics.map(detail => <span key={detail}>{detail}</span>)}</li>)}</ul>{selectedModels[id]?.length > 0 && <><strong>{zh ? "已报告模型" : "Reported models"}</strong><ul>{selectedModels[id].map(model => <li key={model}><code>{model}</code></li>)}</ul><div className="model-test"><label>{zh ? "选择模型" : "Model"}<select value={selectedModel ?? ""} onChange={event => setModelToTest(current => ({ ...current, [id]: event.target.value }))}>{selectedModels[id].map(model => <option key={model} value={model}>{model}</option>)}</select></label><button disabled={!selectedModel || modelTests[selectedModel ?? ""] === "VERIFYING"} onClick={() => selectedModel && onVerifyModel(selectedModel)}>{modelTests[selectedModel ?? ""] === "VERIFYING" ? zh ? "测试中…" : "Testing…" : zh ? "测试此模型（可能消耗额度）" : "Test model (may use quota)"}</button>{modelTests[selectedModel ?? ""] === "VERIFIED" && <span role="status">{zh ? "真实短请求已通过" : "Real short request passed"}</span>}{modelTests[selectedModel ?? ""] === "FAILED" && <span role="status">{zh ? "测试失败，查看页面错误" : "Test failed; see page error"}</span>}</div></>}{buckets.length > 0 && <p>{buckets.map(bucket => `${bucket.label} · ${zh ? "上次观测" : "Observed"} ${new Date(bucket.observed_at * 1000).toLocaleString(zh ? "zh-CN" : "en-US")}${bucket.reset_at ? ` · ${zh ? "重置" : "Resets"} ${new Date(bucket.reset_at * 1000).toLocaleString(zh ? "zh-CN" : "en-US")}` : ""}`).join(" / ")}</p>}</details>
      </article>;
    })}</div>
  </section>;
}
