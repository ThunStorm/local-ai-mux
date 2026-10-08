import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Record = { sequence: number; id: string; source: string; endpoint: string; requested_model: string; resolved_model: string | null; adapter_id: string | null; response_id: string | null; conversation_id: string | null; streamed: boolean; started_at: number; finished_at: number | null; elapsed_ms: number | null; status: string; error_code: string | null; finish_reason: string | null; actual_model: string | null; input_tokens: number | null; output_tokens: number | null; total_tokens: number | null };
const states = ["in_progress", "completed", "failed", "timeout", "cancelled", "truncated", "filtered", "interrupted"];
const zhStates = ["运行中", "已完成", "失败", "超时", "已取消", "已截断", "内容过滤", "异常中断"];
const sources = ["api", "playground", "model_test"];
const zhSources = ["外部 API", "桌面试对话", "模型短测试"];

export default function CallRecords({ locale }: { locale: "zh" | "en" }) {
  const zh = locale === "zh";
  const [recording, setRecording] = useState<boolean | null>(null);
  const [recordingBusy, setRecordingBusy] = useState(false);
  const [recordingError, setRecordingError] = useState("");
  useEffect(() => {
    let alive = true;
    invoke<{ enabled: boolean }>("call_recording_get").then(value => { if (alive) setRecording(value.enabled); }).catch(() => { if (alive) setRecordingError(zh ? "读取录制设置失败，请重试。" : "Could not load the recording setting."); });
    return () => { alive = false; };
  }, []);
  async function setCapture(enabled: boolean) {
    setRecordingBusy(true); setRecordingError("");
    try { const value = await invoke<{ enabled: boolean }>("call_recording_set", { enabled }); setRecording(value.enabled); }
    catch { setRecording(null); setRecordingError(zh ? "保存结果未确认，请重新读取录制状态。" : "Save outcome unknown. Reload the recording setting."); }
    finally { setRecordingBusy(false); }
  }
  async function retryRecording() {
    setRecordingBusy(true); setRecordingError("");
    try { const value = await invoke<{ enabled: boolean }>("call_recording_get"); setRecording(value.enabled); }
    catch { setRecordingError(zh ? "读取录制设置失败，请重试。" : "Could not load the recording setting."); }
    finally { setRecordingBusy(false); }
  }
  const [records, setRecords] = useState<Record[]>([]);
  const [source, setSource] = useState("");
  const [status, setStatus] = useState("");
  const [draft, setDraft] = useState("");
  const [model, setModel] = useState("");
  const [conversation, setConversation] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [more, setMore] = useState(false);
  const [error, setError] = useState("");
  const [confirm, setConfirm] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const generation = useRef(0);
  const latestFilter = useRef({ source, status, model, conversation });
  latestFilter.current = { source, status, model, conversation };
  const labelStatus = (value: string) => zh ? zhStates[states.indexOf(value)] ?? value : value;
  const labelSource = (value: string) => zh ? zhSources[sources.indexOf(value)] ?? value : value;
  const date = (value: number) => new Date(value * 1000).toLocaleString(zh ? "zh-CN" : "en-US");
  async function load(before?: number) {
    const token = ++generation.current;
    setBusy(true); setError("");
    try {
      const filter = latestFilter.current;
      const page = await invoke<Record[]>("call_records_list", { filter: { before: before ?? null, source: filter.source || null, status: filter.status || null, model: filter.model || null, conversation_id: filter.conversation } });
      if (token !== generation.current) return;
      setRecords(current => before ? [...current, ...page.slice(0, 50)] : page.slice(0, 50));
      setMore(page.length > 50);
    } catch { if (token === generation.current) setError(zh ? "读取记录失败，请重试。" : "Could not load records. Retry."); }
    finally { if (token === generation.current) setBusy(false); }
  }
  useEffect(() => { setRecords([]); setMore(false); setSelected(null); setConfirm(null); void load(); return () => { generation.current++; }; }, [source, status, model, conversation]);
  async function remove() {
    if (!confirm) return;
    setBusy(true); setError("");
    try { await invoke("call_records_delete", { id: confirm === "all" ? null : confirm }); setConfirm(null); setSelected(null); await load(); }
    catch { setError(zh ? "清理记录失败，请重试。" : "Could not delete records. Retry."); setBusy(false); }
  }
  return <section aria-label={zh ? "调用历史" : "Call history"}>
    <div className="call-recording-control"><label><input type="checkbox" role="switch" checked={recording === true} disabled={recording === null || recordingBusy} onChange={event => void setCapture(event.target.checked)} /><span>{zh ? "记录对话内容" : "Record dialogue content"}</span></label><span className="call-recording-state" role="status">{recordingBusy ? zh ? "正在保存…" : "Saving…" : recording === null ? recordingError ? zh ? "录制状态未知" : "Recording unknown" : zh ? "正在读取…" : "Loading…" : recording ? zh ? "录制已开启" : "Recording on" : zh ? "录制已关闭" : "Recording off"}</span></div>
    <p className="call-records-description">{zh ? "开启后录制新调用的完整输入与回答（凭据会脱敏）；关闭立即停止正文录制，已录内容保留。关闭时仍记录调用信息；store:false 不保存。" : "Records full inputs and answers for new calls, with credential redaction. Turning off stops active capture and keeps saved text. Call metadata continues; store:false opts out."}</p>
    {recordingError && <div role="alert"><p>{recordingError}</p>{recording === null && <button disabled={recordingBusy} onClick={() => void retryRecording()}>{zh ? "重新读取状态" : "Reload setting"}</button>}</div>}
    <form className="call-filters" onSubmit={event => { event.preventDefault(); setModel(draft.trim()); }}>
      <div className="call-filter-fields">
      <label className="call-select-label">{zh ? "来源" : "Source"}<select aria-label={zh ? "来源" : "Source"} value={source} onChange={e => setSource(e.target.value)}><option value="">{zh ? "全部来源" : "All sources"}</option>{sources.map(value => <option key={value} value={value}>{labelSource(value)}</option>)}</select></label>
      <label className="call-select-label">{zh ? "状态" : "Status"}<select aria-label={zh ? "状态" : "Status"} value={status} onChange={e => setStatus(e.target.value)}><option value="">{zh ? "全部状态" : "All states"}</option>{states.map(value => <option key={value} value={value}>{labelStatus(value)}</option>)}</select></label>
      <label>{zh ? "型号" : "Model"}<input aria-label={zh ? "型号" : "Model"} value={draft} onChange={e => setDraft(e.target.value)} placeholder={zh ? "型号关键词" : "Model keyword"} /></label>
      </div>
      <div className="call-filter-actions">
      <button className="primary" disabled={busy}>{zh ? "查询" : "Search"}</button><button type="button" disabled={busy} onClick={() => void load()}>{zh ? "刷新" : "Refresh"}</button><button className="call-clear" type="button" disabled={busy || records.length === 0} onClick={() => setConfirm("all")}>{zh ? "清理已结束记录" : "Clear finished records"}</button>
      </div>
    </form>
    {conversation && <p>{zh ? "当前会话" : "Conversation"}: {conversation} <button onClick={() => setConversation(null)}>{zh ? "查看全部调用" : "Show all calls"}</button></p>}
    {confirm && <div className="call-confirm" role="alert"><p>{zh ? confirm === "all" ? "清理全部已结束的调用记录？这包含筛选范围外的记录；续接会话映射仍保留。" : "删除这条调用记录？续接会话映射仍保留。" : confirm === "all" ? "Delete all finished calls, including those outside this filter? Resume mappings are retained." : "Delete this call? Resume mappings are retained."}</p><button disabled={busy} onClick={() => void remove()}>{zh ? "确认清理" : "Confirm deletion"}</button><button disabled={busy} onClick={() => setConfirm(null)}>{zh ? "取消" : "Cancel"}</button></div>}
    {error && <p role="alert">{error}</p>}
    {records.length === 0 && !error && <p className="empty" role="status">{busy ? zh ? "正在读取…" : "Loading…" : zh ? "暂无符合条件的调用记录。升级前的 Chat 调用不会补录。" : "No matching calls. Earlier Chat calls are not backfilled."}</p>}
    <div className="session-list">{records.map(record => <article className="card" key={record.id}><strong>{record.resolved_model ?? record.requested_model}</strong><span>{labelSource(record.source)} · {record.endpoint === "model_test" ? zh ? "短测试" : "Test" : record.endpoint === "responses" ? "Responses" : "Chat"} · {labelStatus(record.status)} · {record.streamed ? zh ? "流式" : "Stream" : zh ? "非流式" : "Non-stream"}</span><small>{date(record.started_at)} · {record.elapsed_ms === null ? zh ? "耗时未知" : "Duration unknown" : `${(record.elapsed_ms / 1000).toFixed(1)} s`} · {zh ? "总用量" : "Total tokens"} {record.total_tokens ?? (zh ? "未知" : "Unknown")}{record.error_code ? ` · ${record.error_code}` : ""}</small><div className="call-actions"><button onClick={() => setSelected(selected === record.id ? null : record.id)} aria-expanded={selected === record.id}>{zh ? "详情" : "Details"}</button>{record.conversation_id && <button disabled={busy} onClick={() => setConversation(record.conversation_id)}>{zh ? "本会话调用" : "Conversation calls"}</button>}<button disabled={busy || record.status === "in_progress"} onClick={() => setConfirm(record.id)}>{zh ? "删除记录" : "Delete record"}</button></div>{selected === record.id && <dl className="call-detail"><dt>{zh ? "调用 ID" : "Call ID"}</dt><dd>{record.id}</dd><dt>{zh ? "请求型号" : "Requested model"}</dt><dd>{record.requested_model}</dd><dt>{zh ? "上游报告型号" : "Reported model"}</dt><dd>{record.actual_model ?? (zh ? "未知" : "Unknown")}</dd><dt>{zh ? "连接" : "Connection"}</dt><dd>{record.adapter_id ?? (zh ? "尚未解析" : "Unresolved")}</dd><dt>{zh ? "响应 / 会话 ID" : "Response / conversation ID"}</dt><dd>{record.response_id ?? "—"}<br />{record.conversation_id ?? "—"}</dd><dt>{zh ? "输入 / 输出用量" : "Input / output tokens"}</dt><dd>{record.input_tokens ?? "?"} / {record.output_tokens ?? "?"}</dd><dt>{zh ? "结束原因" : "Finish reason"}</dt><dd>{record.finish_reason ?? (zh ? "未知" : "Unknown")}</dd><dt>{zh ? "结束时间" : "Finished at"}</dt><dd>{record.finished_at === null ? "—" : date(record.finished_at)}</dd>{record.status === "cancelled" && <><dt>{zh ? "取消范围" : "Cancellation"}</dt><dd>{zh ? "仅停止本机接收；上游可能继续执行或计费。" : "Local receive stopped; upstream execution or billing may continue."}</dd></>}</dl>}{selected === record.id && <RecordedDialogue id={record.id} locale={locale} status={record.status} recording={recording} />}</article>)}</div>
    {more && <button className="call-more" disabled={busy} onClick={() => void load(records[records.length - 1]?.sequence)}>{zh ? "加载更多" : "Load more"}</button>}
  </section>;
}


type Dialogue = { messages: { role?: string; content?: unknown }[]; answer: string; state: string };
function RecordedDialogue({ id, locale, status, recording }: { id: string; locale: "zh" | "en"; status: string; recording: boolean | null }) {
  const zh = locale === "zh";
  const [content, setContent] = useState<Dialogue | null>(null);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const token = useRef(0);
  async function reload() {
    const generation = ++token.current; setBusy(true); setError("");
    try { const value = await invoke<Dialogue | null>("call_record_content", { id }); if (token.current === generation) setContent(value); }
    catch { if (token.current === generation) setError(zh ? "读取对话内容失败，请重试。" : "Could not load the dialogue. Retry."); }
    finally { if (token.current === generation) setBusy(false); }
  }
  useEffect(() => { void reload(); return () => { token.current++; }; }, [id, status, recording]);
  const role = (value?: string) => zh ? ({ system: "系统提示词", developer: "开发者指令", user: "用户", assistant: "模型" }[value ?? "user"] ?? value ?? "输入") : value ?? "Input";
  const state = zh ? ({ recording: "录制中", complete: "完整录制", partial: "部分内容", stopped: "已停止录制" }[content?.state ?? ""] ?? "") : content?.state;
  return <section className="recorded-dialogue" aria-label={zh ? "对话内容" : "Recorded dialogue"}><header><h3>{zh ? "对话内容" : "Dialogue content"}</h3><span>{state}</span><button disabled={busy} onClick={() => void reload()}>{zh ? "刷新内容" : "Refresh content"}</button></header>{busy && <p role="status">{zh ? "正在读取对话…" : "Loading dialogue…"}</p>}{error ? <p role="alert">{error}</p> : !busy && !content ? <p className="muted">{zh ? "此调用未录制正文。只有开启开关后的新调用才会保存，旧记录无法补回。" : "This call has no recorded text. Only new calls started while recording is on are captured."}</p> : content && <div className="dialogue-scroll">{Array.isArray(content.messages) ? content.messages.map((message, index) => <article className="dialogue-message" key={index}><strong>{role(message.role)}</strong><p>{typeof message.content === "string" ? message.content : JSON.stringify(message.content)}</p></article>) : <pre>{JSON.stringify(content.messages, null, 2)}</pre>}<article className="dialogue-message dialogue-answer"><strong>{zh ? "模型回答" : "Model answer"}</strong><p>{content.answer || (zh ? "尚无录下的回答。" : "No captured answer yet.")}</p></article>{content.state !== "complete" && <small>{zh ? "这些是已录下的内容，可能不完整；不会自动用于后续上下文。" : "Captured content may be incomplete and is not automatically used as context."}</small>}</div>}</section>;
}
