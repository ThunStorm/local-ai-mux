import { useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import SoftwareIcon from "./SoftwareIcon";
import { buildChatPayload, DEFAULT_PARAMETERS, errorText, eventBelongsToRequest, type ChatEvent, type ChatModel, type Exchange, type Parameters } from "./chat-state";
type Props = { visible: boolean; running: boolean; connections: string; locale: "zh" | "en"; onApi: () => void };
type Active = { id: string; model: string; sequence: number };

export default function ChatPlayground({ visible, running, connections, locale, onApi }: Props) {
  const zh = locale === "zh";
  const [models, setModels] = useState<ChatModel[]>([]);
  const [modelId, setModelId] = useState("");
  const [mode, setMode] = useState<"single" | "multi">("single");
  const [system, setSystem] = useState("");
  const [parameters, setParameters] = useState<Parameters>({ ...DEFAULT_PARAMETERS });
  const [stream, setStream] = useState(true);
  const [draft, setDraft] = useState("");
  const [exchanges, setExchanges] = useState<Exchange[]>([]);
  const [archives, setArchives] = useState<{ model: string; exchanges: Exchange[] }[]>([]);
  const [busy, setBusy] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [inspector, setInspector] = useState(false);
  const [message, setMessage] = useState("");
  const active = useRef<Active | null>(null);
  const mounted = useRef(true);
  const messages = useRef<HTMLDivElement>(null);
  const followTail = useRef(true);
  const paramsButton = useRef<HTMLButtonElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const selected = models.find(model => model.id === modelId);
  useEffect(() => {
    const subscription = listen("localmux-exit-blocked", () => setMessage(zh ? "请等待当前任务完成，或先停止请求后再退出。" : "Wait for the active task, or stop it before quitting."));
    return () => { void subscription.then(unlisten => unlisten()); };
  }, [zh]);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; if (active.current) void invoke("chat_cancel", { requestId: active.current.id }).catch(() => {}); };
  }, []);
  useEffect(() => {
    if (!visible || !running) { if (!running) setModels([]); return; }
    let keep = true;
    setRefreshing(true);
    invoke<{ data: ChatModel[] }>("chat_models").then(value => { if (keep) { setModels(value.data); setMessage(""); } }).catch(cause => { if (keep) setMessage(errorText(String(cause), zh)); }).finally(() => { if (keep) setRefreshing(false); });
    return () => { keep = false; };
  }, [visible, running, connections, zh]);
  useEffect(() => {
    const node = messages.current;
    if (visible && node && followTail.current) node.scrollTop = node.scrollHeight;
  }, [exchanges, visible]);
  useEffect(() => {
    if (inspector && window.matchMedia("(max-width: 1050px)").matches) closeButton.current?.focus();
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape" && inspector && visible) { setInspector(false); paramsButton.current?.focus(); } };
    document.addEventListener("keydown", escape);
    return () => document.removeEventListener("keydown", escape);
  }, [inspector, visible]);

  const refresh = async () => {
    setRefreshing(true); setMessage("");
    try { setModels((await invoke<{ data: ChatModel[] }>("chat_models")).data); }
    catch (cause) { setMessage(errorText(String(cause), zh)); }
    finally { setRefreshing(false); }
  };
  function changeContext(id: string, nextMode?: "single" | "multi") {
    if (active.current) return;
    followTail.current = true;
    if (exchanges.length) setArchives(current => [...current, { model: modelId, exchanges }]);
    const model = models.find(item => item.id === id);
    setModelId(id); setMode(nextMode ?? (model?.chat.history === "SUPPORTED" ? "multi" : "single"));
    setExchanges([]); setSystem(""); setParameters({ ...DEFAULT_PARAMETERS }); setStream(model?.stream ?? false); setMessage("");
  }
  function newConversation() {
    if (busy) return;
    if ((exchanges.length || archives.length || draft) && !window.confirm(zh ? "清除本次运行的对话和草稿？" : "Clear this run's conversations and draft?")) return;
    setExchanges([]); setArchives([]); setDraft(""); setSystem(""); setMessage("");
  }
  async function send() {
    if (active.current || !selected || !running) return;
    let payload;
    try { payload = buildChatPayload(selected, exchanges, draft, mode, system, parameters, stream); }
    catch (cause) { setMessage(cause instanceof Error ? cause.message : zh ? "请求无效" : "Invalid request"); return; }
    const id = crypto.randomUUID();
    followTail.current = true;
    const text = draft;
    const current: Active = { id, model: selected.id, sequence: 0 };
    active.current = current; setBusy(true); setMessage(""); setDraft("");
    setExchanges(history => [...history, { id, user: text, answer: "", status: "waiting", model: selected.id, elapsedMs: 0, error: "", mode, payload }]);
    const events = new Channel<ChatEvent>();
    events.onmessage = event => {
      const request = active.current;
      if (!mounted.current || !request || !eventBelongsToRequest(event, request.id, request.model, request.sequence)) return;
      request.sequence = event.sequence;
      setExchanges(history => history.map(exchange => exchange.id !== id ? exchange : { ...exchange, elapsedMs: event.elapsedMs, answer: exchange.answer + (event.kind === "delta" ? event.text : ""), status: event.kind === "delta" ? "receiving" : event.kind === "started" ? "waiting" : event.kind, error: event.errorCode }));
      if (["completed", "failed", "stopped"].includes(event.kind)) {
        active.current = null; setBusy(false);
        if (event.kind === "failed") { setMessage(errorText(event.errorCode, zh)); setDraft(value => value || text); }
      }
    };
    try { await invoke("chat_send", { input: { requestId: id, request: payload }, events }); }
    catch (cause) {
      if (mounted.current && active.current?.id === id) {
        const code = String(cause).match(/LMX_[A-Z_]+/)?.[0] ?? "LMX_PROVIDER_UNAVAILABLE";
        setExchanges(history => history.map(exchange => exchange.id === id ? { ...exchange, status: "failed", error: code } : exchange));
        setMessage(errorText(code, zh)); setDraft(value => value || text);
        active.current = null; setBusy(false);
      }
    }
  }
  async function stop() {
    if (!active.current) return;
    try { await invoke("chat_cancel", { requestId: active.current.id }); }
    catch { if (active.current) setMessage(zh ? "停止尚未确认，请等待请求终态。" : "Stop is not confirmed; wait for the terminal state."); }
  }
  async function copy(value: string) {
    try { await navigator.clipboard.writeText(value); setMessage(zh ? "已复制" : "Copied"); }
    catch { setMessage(zh ? "复制失败，请手动选择文字。" : "Copy failed; select the text manually."); }
  }
  let preview = "";
  if (selected) {
    try { preview = JSON.stringify(buildChatPayload(selected, exchanges, draft || (zh ? "（当前输入）" : "(current input)"), mode, system, parameters, stream), null, 2); }
    catch { preview = zh ? "输入或参数未通过校验。" : "Input or parameters are invalid."; }
  }
  const label = (status: Exchange["status"]) => ({ waiting: zh ? "等待响应" : "Waiting", receiving: zh ? "接收中" : "Receiving", completed: zh ? "完成" : "Completed", failed: zh ? "失败" : "Failed", stopped: zh ? "已停止本机接收" : "Local reception stopped" })[status];

  const exchangeView = (exchange: Exchange, readonly = false) => <article className="chat-exchange" key={exchange.id}><div className="chat-user"><small>{zh ? "你" : "You"}</small><p>{exchange.user}</p></div><div className="chat-answer"><div className="chat-answer-heading"><SoftwareIcon id={exchange.model.split("/")[0]} small /><strong>{exchange.model}</strong></div>{exchange.answer ? <p>{exchange.answer}</p> : <p className="chat-pending">{label(exchange.status)}</p>}<div className="chat-receipt"><span>{label(exchange.status)} · {(exchange.elapsedMs / 1000).toFixed(1)} s · {exchange.mode === "single" ? zh ? "未携带历史" : "No history sent" : zh ? "独立模型上下文" : "Isolated model context"}</span>{exchange.answer && <button className="quiet" onClick={() => copy(exchange.answer)}>{zh ? "复制" : "Copy"}</button>}</div>{exchange.status === "stopped" && <small className="chat-warning">{zh ? "上游可能继续执行；部分回答不进入后续历史。" : "The upstream may continue; partial text is excluded from history."}</small>}{exchange.error && <p className="chat-error" role="status">{errorText(exchange.error, zh)}</p>}{!readonly && <details className="chat-request"><summary>{zh ? "本次请求" : "Request"}</summary><pre>{JSON.stringify(exchange.payload, null, 2)}</pre><small>{zh ? "用量与费用未知；本机耗时不代表供应商计费。" : "Usage and cost unknown; local duration is not billing data."}</small></details>}</div></article>;

  return <section className={`chat-page${inspector ? " with-inspector" : ""}`} hidden={!visible}>
    <div className="chat-main"><header className="page-heading"><div><h1>{zh ? "试对话" : "Playground"}</h1><p>{zh ? "使用已接入模型，验证本机 API" : "Try connected models through the local API"}</p></div><button disabled={busy} onClick={newConversation}>{zh ? "新对话" : "New chat"}</button></header>
      <div className="chat-toolbar glass"><select aria-label={zh ? "对话模型" : "Chat model"} value={modelId} disabled={!running || busy || refreshing} onChange={event => changeContext(event.target.value)}><option value="">{zh ? "请选择模型" : "Choose a model"}</option>{modelId && !selected && <option value={modelId}>{zh ? "此模型已不可用" : "Model unavailable"} · {modelId}</option>}{models.map(model => <option key={model.id} value={model.id}>{model.id}</option>)}</select><span className="chat-mode">{mode === "multi" ? zh ? "多轮对话" : "Multi-turn" : zh ? "单轮试调用" : "Single-turn"}</span><button className="quiet" disabled={!running || busy || refreshing} onClick={refresh} title={zh ? "刷新模型目录，不发送消息" : "Refresh models without sending"} aria-label={zh ? "刷新模型目录" : "Refresh models"}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7"><path d="M20 7v5h-5M4 17v-5h5M6 7a7 7 0 0 1 13 2M18 17a7 7 0 0 1-13-2" /></svg></button><button ref={paramsButton} className="quiet" aria-expanded={inspector} aria-controls="chat-inspector" onClick={() => setInspector(value => !value)}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7"><path d="M3 6h18M3 12h18M3 18h18"/><circle cx="8" cy="6" r="2"/><circle cx="16" cy="12" r="2"/><circle cx="9" cy="18" r="2"/></svg><span>{zh ? "参数" : "Parameters"}</span></button></div>
      <p className="chat-capability">{selected ? selected.chat.source : running ? zh ? "选择当前 Key 授权的型号；模型目录不代表真实调用已验收。" : "Choose a model authorized for this key; listing is not a live test." : zh ? "本机 API 未启动。" : "Local API is stopped."}</p>
      <div className="chat-messages" ref={messages} onScroll={event => { const node = event.currentTarget; followTail.current = node.scrollHeight - node.clientHeight - node.scrollTop < 96; }} role="log" aria-label={zh ? "对话消息" : "Chat messages"}>{archives.map((archive, index) => <details className="chat-archive" key={index}><summary>{zh ? "旧上下文 · 只读" : "Previous context · Read-only"} · {archive.model}</summary>{archive.exchanges.map(exchange => exchangeView(exchange, true))}</details>)}{exchanges.length ? exchanges.map(exchange => exchangeView(exchange)) : <div className="chat-empty"><svg viewBox="0 0 48 48" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M9 10h30v23H20l-11 7z"/><path d="M17 19h14M17 25h9"/></svg><h2>{!running ? zh ? "先启动本机 API" : "Start the local API" : models.length === 0 ? zh ? "暂无可调用模型" : "No callable models" : zh ? "开始一次试对话" : "Start a conversation"}</h2><p>{!running ? zh ? "接入模型后，用当前 Key 的权限进行调用。" : "Calls use the current key's permissions." : models.length === 0 ? zh ? "检查连接与 Key 权限；Agent 调用须在 API 页显式授权。" : "Check connections and key permissions; agent access needs explicit authorization." : zh ? "选择模型，输入消息。参数默认沿用上游设置。" : "Choose a model and enter a message. Parameters use upstream defaults."}</p>{(!running || !models.length) && <button onClick={onApi}>{zh ? "前往 API 设置" : "Open API settings"}</button>}</div>}</div>
      {message && <p className="chat-feedback" role="status">{message}</p>}<div className="chat-compose"><textarea aria-label={zh ? "消息输入" : "Message input"} value={draft} disabled={!running || !selected} placeholder={zh ? "输入消息，⌘ Enter 发送" : "Message, ⌘ Enter to send"} onChange={event => setDraft(event.target.value)} onKeyDown={event => { if (event.key === "Enter" && (event.metaKey || event.ctrlKey) && !event.nativeEvent.isComposing) { event.preventDefault(); if (!busy) void send(); } }} /><button className="primary" disabled={!busy && (!running || !selected || !draft.trim())} onClick={() => busy ? void stop() : void send()}>{busy ? zh ? "停止" : "Stop" : zh ? "发送" : "Send"}</button><small>{zh ? "正文仅保留本次运行 · 发送可能消耗额度" : "Text stays in this run's memory · Sending may use quota"}</small></div>
    </div>
    {inspector && <aside className="chat-inspector" id="chat-inspector" aria-label={zh ? "对话参数" : "Chat parameters"}><header><h2>{zh ? "参数" : "Parameters"}</h2><button className="quiet" disabled={busy} onClick={() => { setParameters({ ...DEFAULT_PARAMETERS }); setStream(selected?.stream ?? false); }}>{zh ? "恢复默认" : "Reset"}</button><button className="quiet inspector-close" ref={closeButton} aria-label={zh ? "关闭参数" : "Close parameters"} onClick={() => { setInspector(false); paramsButton.current?.focus(); }}>×</button></header><fieldset disabled={busy || !selected}><label>{zh ? "对话模式" : "Chat mode"}<select value={mode} onChange={event => changeContext(modelId, event.target.value as "single" | "multi")}><option value="single">{zh ? "单轮试调用" : "Single-turn"}</option><option value="multi" disabled={selected?.chat.history !== "SUPPORTED"}>{zh ? "多轮对话" : "Multi-turn"}</option></select></label><label>{zh ? "系统提示词" : "System prompt"}<textarea value={system} disabled={mode !== "multi" || selected?.chat.system !== "SUPPORTED"} placeholder={zh ? "可选" : "Optional"} onChange={event => setSystem(event.target.value)} /></label>{mode === "single" && <small>{zh ? "仅发送当前消息；不携带历史或系统提示词。" : "Only the current message is sent; no history or system prompt."}</small>}
      {(["temperature", "max_tokens", "top_p"] as const).map(key => {
        const capability = selected?.chat[key]; const enabled = capability?.support === "SUPPORTED" && capability.max !== null;
        const value = parameters[key]; const defaultValue = key === "max_tokens" ? Math.min(2048, capability?.max ?? 2048) : key === "top_p" ? 1 : .7;
        const name = key === "temperature" ? zh ? "采样温度" : "Temperature" : key === "max_tokens" ? zh ? "最大输出长度" : "Output limit" : "Top P";
        return <div className={`chat-parameter${enabled ? "" : " unavailable"}`} key={key}><div className="chat-parameter-head"><label><input type="checkbox" checked={value !== null} disabled={!enabled} onChange={event => setParameters(current => ({ ...current, [key]: event.target.checked ? defaultValue : null, ...(event.target.checked && key === "temperature" ? { top_p: null } : event.target.checked && key === "top_p" ? { temperature: null } : {}) }))} />{name}</label><input aria-label={name} type="number" value={Number.isFinite(value ?? defaultValue) ? value ?? defaultValue : ""} min={capability?.min ?? 0} max={capability?.max ?? undefined} step={key === "max_tokens" ? 1 : .05} disabled={!enabled || value === null} onChange={event => setParameters(current => ({ ...current, [key]: Number(event.target.value) }))} /></div>{key !== "max_tokens" && <input aria-label={`${name} ${zh ? "滑块" : "slider"}`} type="range" min={capability?.min ?? 0} max={capability?.max ?? 1} step={.05} value={value ?? defaultValue} disabled={!enabled || value === null} onChange={event => setParameters(current => ({ ...current, [key]: Number(event.target.value) }))} />}<small>{enabled ? value !== null ? zh ? "下次请求提交该值；上游可能按自身规则拒绝。" : "Sent on the next request; upstream rules may reject it." : zh ? "未开启，使用模型默认值。" : "Off: use model default." : capability?.support === "UNSUPPORTED" ? zh ? "当前连接不支持。" : "Unsupported by this connection." : zh ? "能力尚未报告，暂不可调节。" : "Capability unknown; disabled."}</small></div>;
      })}<label className="chat-stream"><input type="checkbox" checked={stream} disabled={!selected?.stream} onChange={event => setStream(event.target.checked)} />{zh ? "流式输出" : "Stream output"}</label></fieldset><ModelCapabilities model={selected} zh={zh} /><details className="chat-preview"><summary>{zh ? "请求预览" : "Request preview"}</summary><pre>{preview}</pre><small>{zh ? "不包含 Key；不执行请求。" : "No key; does not execute a request."}</small></details></aside>}
  </section>;
}

function ModelCapabilities({ model, zh }: { model?: ChatModel; zh: boolean }) {
  const [now, setNow] = useState(() => Date.now() / 1000);
  useEffect(() => { const timer = window.setInterval(() => setNow(Date.now() / 1000), 10_000); return () => window.clearInterval(timer); }, []);
  if (!model) return null;
  const names: { [name: string]: string } = { text_input: "文字输入", image_input: "图片输入", non_stream: "非流式", sse: "流式", system: "系统指令", developer: "开发者指令", history: "无状态历史", session_resume: "原生续接", temperature: "温度", top_p: "采样范围", max_tokens: "输出上限", max_completion_tokens: "完整输出上限", reasoning_effort: "推理档位", context_window: "上下文上限", json_object: "JSON 对象约束", json_schema: "JSON Schema 约束", usage: "用量元数据", finish_reason: "结束原因元数据", cancellation: "远端取消" };
  const label = (value: string) => zh ? ({ SUPPORTED: "支持", UNSUPPORTED: "不支持", UNKNOWN: "未知" }[value] ?? "未知") : value;
  return <details className="chat-preview"><summary>{zh ? "型号能力与依据" : "Model capabilities and evidence"}</summary><p>{model.catalog && now >= model.catalog.expires_at ? zh ? "目录已过期，请刷新；不能据此确认能力。" : "Catalog expired; refresh before relying on capabilities." : zh ? "目录与接口映射不代表真实模型验收。" : "Catalog and interface mapping are not live model acceptance."}</p>{model.chat.details ? Object.entries(model.chat.details).map(([name, detail]) => {
    const current = detail.freshness !== "STALE" && detail.evidence.some(e => e.observed_at != null && e.observed_at <= now && now < (e.expires_at ?? e.observed_at + 300));
    const state = current ? detail.effective_support : "UNKNOWN";
    return <p key={name}><strong>{zh ? names[name] ?? "其他能力" : name} · {label(state)}</strong><br /><small>{zh ? "原生 / 公开接口" : "Native / public interface"} {label(detail.native_support)} / {label(detail.public_interface_support)} · {current ? zh ? "当前观察" : "Current observation" : zh ? "过期或时间未知" : "Stale or undated"}<br />{detail.evidence.map((e, i) => <span key={i}>{zh ? "接口版本" : "Interface version"} {e.version ?? (zh ? "未知" : "Unknown")} · {e.live_verified ? zh ? "已授权实测" : "Authorized live test" : zh ? "公开契约或离线依据，未实测" : "Contract/fixture, not live verified"}</span>)}</small></p>;
  }) : <p>{zh ? "旧目录没有分层证据，来源未知。" : "Legacy catalog has no layered evidence."}</p>}</details>;
}
