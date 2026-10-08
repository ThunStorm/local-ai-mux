import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Props = { running: boolean; baseUrl: string; connections: string; locale: "zh" | "en" };
type KeyStatus = "ABSENT" | "READY" | "MISSING";
type Example = "json" | "curl" | "python" | "typescript";
const examples: Example[] = ["json", "curl", "python", "typescript"];

export default function ApiQuickSetup({ running, baseUrl, connections, locale }: Props) {
  const zh = locale === "zh";
  const [status, setStatus] = useState<KeyStatus>("ABSENT");
  const [models, setModels] = useState<string[]>([]);
  const [model, setModel] = useState(() => localStorage.getItem("localmux.selectedModel") ?? "");
  const [shownKey, setShownKey] = useState("");
  const [readOnlyAgent, setReadOnlyAgent] = useState(false);
  const [agentAllowed, setAgentAllowed] = useState(false);
  const [verified, setVerified] = useState<string[]>([]);
  const [exampleKind, setExampleKind] = useState<Example>("json");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const agentConnected = /(^|\|)(codex|qoder|qoder-cn|codebuddy):/.test(connections);

  useEffect(() => {
    let active = true;
    setVerified([]);
    invoke<KeyStatus>("default_api_key_status").then(value => { if (active) setStatus(value); }).catch(() => {});
    invoke<boolean>("default_api_key_allows_agent").then(value => { if (active) { setAgentAllowed(value); setReadOnlyAgent(value); } }).catch(() => {});
    if (running) {
      invoke<string[]>("api_setup_models").then(value => { if (active) setModels(value); }).catch(cause => { if (active) setMessage(String(cause)); });
    } else {
      setModels([]);
    }
    return () => { active = false; };
  }, [running, connections]);

  const selected = models.includes(model) ? model : "";
  const copy = async (value: string) => { await navigator.clipboard.writeText(value); setMessage(zh ? "已复制" : "Copied"); };
  const currentKey = async () => invoke<string>("reveal_default_api_key");
  const shellQuote = (value: string) => `'${value.replace(/'/g, "'\\''")}'`;
  const example = (kind: Example, key: string, modelId = selected || "YOUR_MODEL") => {
    const payload = JSON.stringify({ model: modelId, messages: [{ role: "user", content: "Hello" }] });
    if (kind === "json") return JSON.stringify({ baseURL: baseUrl, apiKey: key, model: modelId }, null, 2);
    if (kind === "curl") return `curl ${shellQuote(`${baseUrl}/chat/completions`)} -H ${shellQuote(`Authorization: Bearer ${key}`)} -H 'Content-Type: application/json' -d ${shellQuote(payload)}`;
    if (kind === "python") return `from openai import OpenAI\nclient = OpenAI(base_url=${JSON.stringify(baseUrl)}, api_key=${JSON.stringify(key)})\nprint(client.chat.completions.create(model=${JSON.stringify(modelId)}, messages=[{"role":"user","content":"Hello"}]).choices[0].message.content)`;
    return `import OpenAI from "openai";\nconst client = new OpenAI({ baseURL: ${JSON.stringify(baseUrl)}, apiKey: ${JSON.stringify(key)} });\nconst result = await client.chat.completions.create({ model: ${JSON.stringify(modelId)}, messages: [{ role: "user", content: "Hello" }] });\nconsole.log(result.choices[0].message.content);`;
  };
  const run = async (action: () => Promise<void>) => {
    setBusy(true); setMessage("");
    try { await action(); } catch (cause) { setMessage(String(cause)); }
    finally { setBusy(false); }
  };
  const replaceKey = async (allowAgent: boolean) => {
    await invoke("reset_default_api_key", { readOnlyAgent: allowAgent });
    setStatus("READY");
    setReadOnlyAgent(allowAgent);
    setAgentAllowed(allowAgent);
    setShownKey("");
    if (running) setModels(await invoke<string[]>("api_setup_models"));
  };

  const saveAgentPermission = async (allowAgent: boolean) => {
    await invoke("set_default_api_key_agent", { readOnlyAgent: allowAgent });
    setReadOnlyAgent(allowAgent); setAgentAllowed(allowAgent);
    if (running) setModels(await invoke<string[]>("api_setup_models"));
    setMessage(zh ? "授权已保存，Key 保持不变；重启后继续有效。" : "Permission saved; the key stays unchanged across restarts.");
  };

  return <div className="api-setup">
    <section className="config-row">
      <div className="config-label"><h2>Base URL</h2><p>{zh ? "本机 OpenAI 兼容地址" : "Local OpenAI-compatible address"}</p></div>
      <div className="config-control"><code className="config-value" title={baseUrl}>{baseUrl || "…"}</code><button disabled={!baseUrl || busy} onClick={() => run(() => copy(baseUrl))}>{zh ? "复制地址" : "Copy URL"}</button></div>
    </section>
    <section className="config-row">
      <div className="config-label"><h2>LocalAiMux Key</h2><p>{zh ? "仅用于本机认证；重启和重新部署后保持不变" : "Local authentication; retained across restarts and deployments"}</p></div>
      <div className="config-control"><code className="config-value secret-value">{status === "READY" ? shownKey || "••••••••••••••••" : status === "MISSING" ? zh ? "凭据缺失，请在下方重建" : "Credential missing; replace below" : zh ? "启动 API 后创建" : "Created when API starts"}</code><button disabled={status !== "READY" || busy} onClick={() => run(async () => setShownKey(shownKey ? "" : await currentKey()))}>{shownKey ? zh ? "隐藏" : "Hide" : zh ? "显示" : "Show"}</button><button disabled={status !== "READY" || busy} onClick={() => run(async () => copy(await currentKey()))}>{zh ? "复制 Key" : "Copy key"}</button></div>
    </section>
    <section className="config-row">
      <div className="config-label"><h2>{zh ? "模型" : "Model"}</h2><p>{zh ? "当前 Key 可调用的官方报告型号" : "Reported models callable by this key"}</p></div>
      <div className="config-control"><select className="config-value" aria-label={zh ? "模型" : "Model"} value={selected} disabled={!running || models.length === 0} onChange={event => { setModel(event.target.value); localStorage.setItem("localmux.selectedModel", event.target.value); }}><option value="">{running ? zh ? "请选择模型" : "Choose a model" : zh ? "启动服务后查看可用模型" : "Start API to view models"}</option>{models.map(id => <option key={id} value={id}>{id}</option>)}</select><button disabled={!running || busy} onClick={() => run(async () => setModels(await invoke<string[]>("api_setup_models")))}>{zh ? "刷新" : "Refresh"}</button></div>
      <p className="config-hint">{!running ? zh ? "服务未启动，模型暂不可用。" : "The API is stopped; models are unavailable." : models.length === 0 ? agentConnected && !agentAllowed ? zh ? "当前 Key 不允许 Agent；授权只读调用后才会显示已连接的 Agent 模型。" : "This key cannot call agents; allow read-only calls to see agent models." : zh ? "暂无可调用模型；请在应用与连接页刷新连接模型。" : "No callable model. Refresh the connection catalog in Apps & connections." : selected ? verified.includes(selected) ? zh ? "此模型已在本次运行中通过真实短请求。" : "This model passed a real short request in this run." : zh ? "模型已报告，尚未实测。" : "Reported model; not yet tested." : zh ? "选择一个真实模型以生成配置。" : "Choose a reported model to build a configuration."}</p>
      {running && models.length === 0 && agentConnected && !agentAllowed && status === "READY" && <button className="text-action" disabled={busy} onClick={() => run(() => saveAgentPermission(true))}>{zh ? "允许只读 Agent（Key 保持不变）" : "Allow read-only agents (keep the same key)"}</button>}
      {running && selected && <button className="text-action" disabled={busy} onClick={() => run(async () => { const elapsed = await invoke<number>("verify_model", { model: selected }); setVerified(current => [...new Set([...current, selected])]); setMessage(zh ? `验证成功 · ${elapsed} 毫秒` : `Verified · ${elapsed} ms`); })}>{zh ? "测试此模型（会调用上游，可能消耗额度）" : "Test this model (calls upstream; may use quota)"}</button>}
    </section>
    <section className="examples-section">
      <div className="examples-heading"><div><h2>{zh ? "配置示例" : "Configuration examples"}</h2><p>{zh ? "预览隐去 Key；复制完整配置需主动点击" : "The preview hides the key; copying the full config requires a click"}</p></div><button className="primary" disabled={!running || !selected || status !== "READY" || busy} onClick={() => run(async () => copy(example(exampleKind, await currentKey())))}>{zh ? "复制完整配置" : "Copy complete config"}</button></div>
      <div className="example-tabs" role="group" aria-label={zh ? "示例语言" : "Example language"}>{examples.map(kind => <button key={kind} aria-pressed={exampleKind === kind} className={exampleKind === kind ? "active" : ""} onClick={() => setExampleKind(kind)}>{kind === "json" ? "JSON" : kind === "curl" ? "cURL" : kind === "python" ? "Python" : "TypeScript"}</button>)}</div>
      <pre className="example-code"><code>{example(exampleKind, "YOUR_LOCALAIMUX_KEY")}</code></pre>
    </section>
    <details className="inline-disclosure"><summary>{zh ? "本机 Key 权限与重建" : "Local key permissions and replacement"}</summary><div className="disclosure-body"><p>{zh ? "授权保存后跨重启保留，保存授权不改变 Key；只有主动重建才撤销旧 Key。上游运行时仍管理自身工具权限。" : "Permissions persist across restarts without changing the key; explicit replacement revokes the old key. The upstream runtime still controls its tools."}</p><label><input type="checkbox" checked={readOnlyAgent} onChange={event => setReadOnlyAgent(event.target.checked)} />{zh ? "明确允许只读 Agent 调用" : "Allow read-only agent calls"}</label><button disabled={busy || status !== "READY"} onClick={() => run(() => saveAgentPermission(readOnlyAgent))}>{zh ? "保存授权（Key 不变）" : "Save permissions (keep key)"}</button><button disabled={busy} onClick={() => run(() => replaceKey(readOnlyAgent))}>{zh ? "重建本机 Key 并撤销旧 Key" : "Replace local key and revoke old key"}</button></div></details>
    {message && <p className="inline-message" role="status">{message}</p>}
  </div>;
}
