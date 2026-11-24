import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Policy = { retention_days: number; budget_mib: number };
type Status = {
  active_dir: string; anchor_dir: string;
  config: { data_dir: string | null; pending_data_dir: string | null; logs: Policy };
  active_policy: Policy; usage: { category: string; path: string; bytes: number | null }[];
  growth: { elapsed_seconds: number; delta_bytes: number; bytes_per_day: number } | null;
  sampling_ok: boolean; logs: { healthy: boolean };
};
function bytes(value: number | null, zh: boolean): string {
  if (value === null) return zh ? "未知" : "Unknown";
  const sign = value < 0 ? "−" : ""; const n = Math.abs(value);
  return sign + (n >= 1024 ** 3 ? `${(n / 1024 ** 3).toFixed(2)} GiB` : n >= 1024 ** 2 ? `${(n / 1024 ** 2).toFixed(2)} MiB` : n >= 1024 ? `${(n / 1024).toFixed(1)} KiB` : `${n} B`);
}
export default function DataStorage({ locale }: { locale: "zh" | "en" }) {
  const zh = locale === "zh";
  const [status, setStatus] = useState<Status | null>(null);
  const [path, setPath] = useState(""); const [days, setDays] = useState(""); const [budget, setBudget] = useState("");
  const [busy, setBusy] = useState(true); const [error, setError] = useState(""); const [saved, setSaved] = useState(false);
  const generation = useRef(0);
  useEffect(() => { void reload(); return () => { generation.current++; }; }, []);
  const valid = /^\d+$/.test(days) && Number(days) >= 1 && Number(days) <= 90 && /^\d+$/.test(budget) && Number(budget) >= 10 && Number(budget) <= 1024 && path.trim().length > 0;
  const changed = status !== null && (path !== (status.config.pending_data_dir ?? status.active_dir) || Number(days) !== status.config.logs.retention_days || Number(budget) !== status.config.logs.budget_mib);
  const pending = status !== null && (status.config.pending_data_dir !== null || status.config.logs.retention_days !== status.active_policy.retention_days || status.config.logs.budget_mib !== status.active_policy.budget_mib);
  function accept(value: Status) { setStatus(value); setPath(value.config.pending_data_dir ?? value.active_dir); setDays(String(value.config.logs.retention_days)); setBudget(String(value.config.logs.budget_mib)); }
  async function reload() {
    const token = ++generation.current; setBusy(true); setError(""); setSaved(false);
    try { const value = await invoke<Status>("data_storage_status"); if (token === generation.current) accept(value); }
    catch { if (token === generation.current) { setStatus(null); setError(zh ? "配置或占用读取失败，请重新读取。" : "Could not read storage settings or usage. Reload to retry."); } }
    finally { if (token === generation.current) setBusy(false); }
  }
  async function save(reset = false) {
    if (busy || !status || (!reset && !valid)) return;
    const token = ++generation.current; setBusy(true); setError(""); setSaved(false);
    try {
      await invoke("data_storage_save", { path: reset ? status.active_dir : path, retentionDays: reset ? status.active_policy.retention_days : Number(days), budgetMib: reset ? status.active_policy.budget_mib : Number(budget) });
      const value = await invoke<Status>("data_storage_status"); if (token === generation.current) { accept(value); setSaved(true); }
    } catch (e) { if (token === generation.current) { setStatus(null); setError(zh ? `保存结果未确认，请重新读取。${String(e)}` : `Save not confirmed. Reload to verify. ${String(e)}`); } }
    finally { if (token === generation.current) setBusy(false); }
  }
  async function pick() {
    if (busy) return;
    const token = ++generation.current; setBusy(true); setError("");
    try { const value = await invoke<string | null>("data_storage_pick"); if (token === generation.current && value !== null) { setPath(value); setSaved(false); } }
    catch { if (token === generation.current) setError(zh ? "文件夹选择失败，可输入绝对路径。" : "Folder picker failed. Enter an absolute path."); }
    finally { if (token === generation.current) setBusy(false); }
  }
  const names: Record<string, [string, string]> = { database: ["数据库（含正文、WAL/SHM）", "Database (including recordings, WAL/SHM)"], backups: ["人工备份", "Manual backups"], logs: ["诊断日志", "Diagnostic logs"], local_keys: ["本机 Key 安全文件", "Local key files"] };
  return <section className="page data-storage-page"><header className="page-heading"><div><h1>{zh ? "数据与日志" : "Data and logs"}</h1><p>{zh ? "独立存储业务数据，管理诊断日志。" : "Keep application data separate and manage diagnostic logs."}</p></div></header>
    <form className="card storage-form" onSubmit={e => { e.preventDefault(); void save(); }}>
      <h2>{zh ? "数据位置" : "Data location"}</h2>
      <p>{zh ? "当前目录" : "Current directory"}<code className="storage-path">{status?.active_dir ?? (zh ? "尚未读取" : "Not loaded")}</code></p>
      <label>{zh ? "数据文件夹（绝对路径）" : "Data folder (absolute path)"}<input value={path} disabled={busy || !status} onChange={e => { setPath(e.target.value); setSaved(false); }} spellCheck={false} /></label>
      <div className="call-actions"><button type="button" disabled={busy || !status} onClick={() => void pick()}>{zh ? "选择空文件夹…" : "Choose empty folder…"}</button></div>
      <p className="storage-help">{zh ? "保存后下次正常启动迁移数据库、备份和日志；请选择已存在的空目录。迁移校验成功才切换，原目录保留回退副本（额外占用空间、不再更新）。请等待所有任务结束后退出并重新打开应用。" : "On the next normal launch, migrate the database, backups and logs into an existing empty folder. Switch only after validation. The original remains as a rollback copy (extra disk usage, no further updates). Wait for all tasks to finish, then quit and reopen."}</p>
      <h2>{zh ? "日志保留" : "Log retention"}</h2><div className="storage-policy">
        <label>{zh ? "保留天数（1–90）" : "Days to keep (1–90)"}<input type="number" min="1" max="90" step="1" value={days} disabled={busy || !status} onChange={e => { setDays(e.target.value); setSaved(false); }} /></label>
        <label>{zh ? "日志总预算（10–1024 MiB）" : "Log budget (10–1024 MiB)"}<input type="number" min="10" max="1024" step="1" value={budget} disabled={busy || !status} onChange={e => { setBudget(e.target.value); setSaved(false); }} /></label>
      </div><p className="storage-help">{zh ? "5 MiB 或跨 UTC 日轮转并 gzip 压缩，超龄或超预算先清理最老归档；每分钟维护。仅记录脱敏诊断。策略下次启动生效，不清理会话正文或人工备份。" : "Rotate at 5 MiB or a new UTC day and gzip archives. Remove oldest archives when expired or over budget; maintain every minute. Redacted diagnostics only. Policy applies next launch; recordings and manual backups are retained."}</p>
      {status && <p role="status">{zh ? `当前策略：${status.active_policy.retention_days}天 / ${status.active_policy.budget_mib} MiB。` : `Active policy: ${status.active_policy.retention_days} days / ${status.active_policy.budget_mib} MiB.`}{pending ? zh ? " 有待生效配置，需正常重启。" : " Pending changes require a normal restart." : ""}{saved ? zh ? " 配置已保存并读回。" : " Settings saved and read back." : ""}</p>}
      {status && !status.logs.healthy && <p role="alert">{zh ? "文件日志不可用或维护失败，请检查数据盘权限和剩余空间。" : "File logging unavailable or maintenance failed. Check permissions and free space."}</p>}
      {error && <p role="alert">{error}</p>}
      <div className="call-actions"><button type="submit" className="primary" disabled={busy || !status || !valid || !changed}>{busy ? zh ? "处理中…" : "Working…" : zh ? "保存，下次启动生效" : "Save for next launch"}</button><button type="button" disabled={busy} onClick={() => void reload()}>{zh ? "重新读取 / 刷新占用" : "Reload / refresh usage"}</button>{pending && <button type="button" disabled={busy} onClick={() => void save(true)}>{zh ? "取消待生效变更" : "Cancel pending changes"}</button>}</div>
    </form>
    <section className="card storage-usage"><h2>{zh ? "当前占用与增长" : "Current usage and growth"}</h2>
      <dl>{status?.usage.map(item => <div key={item.category}><dt>{names[item.category]?.[zh ? 0 : 1] ?? item.category}<code className="storage-path">{item.path}</code></dt><dd>{bytes(item.bytes, zh)}</dd></div>)}</dl>
      <p>{!status ? zh ? "尚未读取" : "Not loaded" : !status.sampling_ok ? zh ? "增长采样不可用，请检查数据目录。" : "Growth sampling unavailable. Check the data folder." : status.growth ? zh ? `数据库区间净增：${bytes(status.growth.delta_bytes, zh)} / ${(status.growth.elapsed_seconds / 3600).toFixed(1)}小时，折算${bytes(status.growth.bytes_per_day, zh)}/天。` : `Database net change: ${bytes(status.growth.delta_bytes, zh)} over ${(status.growth.elapsed_seconds / 3600).toFixed(1)} hours; ${bytes(status.growth.bytes_per_day, zh)}/day extrapolated.` : zh ? "数据库增长率未知：需至少间隔1小时的采样。" : "Database growth rate unknown: samples must be at least one hour apart."}</p>
      <p className="storage-help">{zh ? "逻辑文件大小，非文件系统分配块；净增不是写入吞吐或未来承诺。采样保留最近30天，启动、每小时及手动刷新时读取。会话正文是否录制会显著影响增长，删除记录不保证数据库立即缩小。" : "Logical file sizes, not allocated blocks. Net change is not write throughput or a growth guarantee. Samples cover 30 days, collected on launch, hourly and manual refresh. Recording affects growth; deleting records may not shrink SQLite immediately."}</p>
      <details><summary>{zh ? "系统与安全存储说明" : "System and secure storage"}</summary><p className="storage-help">{zh ? "本机 API Key 留在固定引导目录，不随数据迁移；storage.json和进程锁也保留在该处。WebView缓存、界面偏好和OS临时文件由系统管理，不随业务数据目录迁移。上游CLI自己的缓存和历史不在此统计。" : "Local API keys, storage.json and the process lock stay in the fixed bootstrap folder. WebView caches/preferences and OS temporary files remain system-managed. Upstream CLI caches/history are excluded."}<code className="storage-path">{status?.anchor_dir}</code></p></details>
    </section>
  </section>;
}
