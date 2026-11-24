import { useEffect, useRef, useState } from "react";
import { parseCliIdleSeconds, MAX_CLI_IDLE_SECONDS } from "./execution-limits";
import { invoke } from "@tauri-apps/api/core";

export function timeoutLayerLabel(layer: string | null | undefined, zh: boolean) {
  const labels: Record<string, [string, string]> = {
    cli_idle: ["CLI 输出事件空闲", "CLI output event idle"], cli_rpc: ["CLI 控制响应", "CLI control response"],
    cli_command: ["CLI 目录 / 命令", "CLI catalog / command"], cli_exit: ["CLI 退出等待", "CLI exit wait"],
    gateway_startup: ["Gateway 连接启动", "Gateway adapter startup"], gateway_stream_idle: ["Gateway 上游事件空闲", "Gateway upstream event idle"],
    gateway_overall: ["Gateway 总期限", "Gateway overall deadline"], provider_http: ["HTTP Adapter 请求", "HTTP adapter request"],
  };
  return labels[layer ?? ""]?.[zh ? 0 : 1] ?? (zh ? "未知" : "Unknown");
}
export default function ExecutionLimits({ locale }: { locale: "zh" | "en" }) {
  const zh = locale === "zh";
  const [saved, setSaved] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState(false);
  const generation = useRef(0);
  useEffect(() => { void reload(); return () => { generation.current++; }; }, []);
  const seconds = Number(draft);
  const valid = parseCliIdleSeconds(draft) !== null;
  async function save() {
    if (!valid || busy) return;
    const token = ++generation.current;
    setBusy(true); setError(false);
    try { const value = await invoke<number>("cli_idle_timeout_set", { seconds }); if (token === generation.current) { setSaved(value); setDraft(String(value)); } }
    catch { if (token === generation.current) { setSaved(null); setError(true); } }
    finally { if (token === generation.current) setBusy(false); }
  }
  async function reload() {
    const token = ++generation.current;
    setBusy(true); setError(false);
    try { const value = await invoke<number>("cli_idle_timeout_get"); if (token === generation.current) { setSaved(value); setDraft(String(value)); } }
    catch { if (token === generation.current) { setSaved(null); setError(true); } }
    finally { if (token === generation.current) setBusy(false); }
  }
  return <details className="inline-disclosure"><summary>{zh ? "调用限时" : "Execution timeouts"}</summary><form className="routing-form" onSubmit={e => { e.preventDefault(); void save(); }}>
    <label>{zh ? "CLI 空闲限时（秒）" : "CLI idle timeout (seconds)"}<input type="number" min="30" max={MAX_CLI_IDLE_SECONDS} step="1" value={draft} disabled={busy || saved === null} onChange={e => setDraft(e.target.value)} aria-describedby="cli-timeout-help" /></label>
    <p id="cli-timeout-help">{zh ? "30–1200 秒，默认 180 秒。等待下一条完整 CLI 输出事件；Agent 的 Gateway 上游事件等待同步使用此值。保存后仅对新请求生效，并在重启后保留。" : "30–1200 seconds; default 180. Waits for the next complete CLI output event. The Agent gateway event wait uses the same value. Saved changes apply to new requests and persist across restarts."}</p>
    <p>{zh ? "总期限为 1200 秒（包含排队）；流式连接启动 30 秒、Codex 控制响应 15 秒。试对话客户端另有 120 秒总期限，其他客户端也可能更早停止等待。延长 CLI 空闲限时不会延长这些限制。" : "The overall budget remains 1200 seconds including queueing; stream startup is 30 seconds and Codex control responses 15 seconds. Playground has its own 120-second deadline; other clients may stop sooner. Increasing CLI idle time does not extend these limits."}</p>
    <small role="status">{saved === null ? zh ? "尚未读取配置" : "Setting not loaded" : zh ? `已保存：${saved} 秒` : `Saved: ${saved} seconds`}</small>
    {error && <p role="alert">{zh ? "配置读取或保存失败，请重新读取后再试。" : "Could not load or save. Reload and try again."}</p>}
    <div className="call-actions"><button type="submit" disabled={busy || saved === null || !valid || seconds === saved}>{busy ? zh ? "处理中…" : "Working…" : zh ? "保存限时" : "Save timeout"}</button><button type="button" disabled={busy} onClick={() => void reload()}>{zh ? "重新读取" : "Reload"}</button></div>
  </form></details>;
}
