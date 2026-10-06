export type Support = "SUPPORTED" | "UNSUPPORTED" | "UNKNOWN";
export type ParameterCapability = { support: Support; min: number; max: number | null };
export type ChatModel = { id: string; provider: string; stream: boolean; chat: { history: Support; system: Support; temperature: ParameterCapability; top_p: ParameterCapability; max_tokens: ParameterCapability; source: string } };
export type Parameters = { temperature: number | null; top_p: number | null; max_tokens: number | null };
export type ChatMessage = { role: "user" | "assistant" | "system"; content: string };
export type ExchangeStatus = "waiting" | "receiving" | "completed" | "failed" | "stopped";
export type Exchange = { id: string; user: string; answer: string; status: ExchangeStatus; model: string; elapsedMs: number; error: string; mode: "single" | "multi"; payload: ChatPayload };
export type ChatPayload = { model: string; messages: ChatMessage[]; stream: boolean; temperature?: number; top_p?: number; max_tokens?: number };
export const DEFAULT_PARAMETERS: Parameters = { temperature: null, top_p: null, max_tokens: null };
export function buildChatPayload(model: ChatModel, history: Exchange[], draft: string, mode: "single" | "multi", system: string, parameters: Parameters, stream: boolean): ChatPayload {
  if (!draft.trim()) throw new Error("请输入消息。");
  if (mode === "multi" && model.chat.history !== "SUPPORTED") throw new Error("当前模型不支持携带历史。");
  if (system && (mode !== "multi" || model.chat.system !== "SUPPORTED")) throw new Error("当前模式不支持系统提示词。");
  if (stream && !model.stream) throw new Error("当前模型不支持流式输出。");
  if (parameters.temperature !== null && parameters.top_p !== null) throw new Error("采样温度和 Top P 请选择一项调整。");
  const payload: ChatPayload = { model: model.id, messages: [], stream };
  if (mode === "multi") {
    if (system) payload.messages.push({ role: "system", content: system });
    for (const exchange of history) {
      if (exchange.status !== "completed") continue;
      if (exchange.model !== model.id) throw new Error("不能把其他模型的历史加入当前请求。");
      payload.messages.push({ role: "user", content: exchange.user }, { role: "assistant", content: exchange.answer });
    }
  }
  payload.messages.push({ role: "user", content: draft });
  for (const key of ["temperature", "top_p", "max_tokens"] as const) {
    const value = parameters[key];
    if (value === null) continue;
    const capability = model.chat[key];
    if (capability.support !== "SUPPORTED" || !Number.isFinite(value) || capability.max === null || value < capability.min || value > capability.max || key === "max_tokens" && !Number.isInteger(value)) throw new Error("参数不受支持或超出已报告范围。");
    payload[key] = value;
  }
  if (new TextEncoder().encode(JSON.stringify(payload)).byteLength > 1024 * 1024) throw new Error("请求超过 1 MiB，请缩短输入或开启新对话；历史没有被自动截断。");
  return payload;
}
export type ChatEvent = { requestId: string; sequence: number; kind: "started" | "delta" | "completed" | "failed" | "stopped"; model: string; text: string; errorCode: string; elapsedMs: number };
export function eventBelongsToRequest(event: ChatEvent, requestId: string, model: string, previousSequence: number): boolean {
  return event.requestId === requestId && event.model === model && Number.isSafeInteger(event.sequence) && event.sequence > previousSequence;
}
export function errorText(code: string, zh: boolean): string {
  const messages: Record<string, [string,string]> = {
    LMX_API_STOPPED: ["请先启动本机 API。", "Start the local API first."],
    LMX_KEY_MISSING: ["本机 Key 不可用，请前往 API 设置。", "The local key is unavailable; open API settings."],
    LMX_PERMISSION_DENIED: ["当前 Key 无调用权限，请前往 API 权限设置。", "This key lacks permission; open API settings."],
    LMX_AUTH_REQUIRED: ["认证失效，请检查本机 Key 或官方连接。", "Authentication expired; check the key or connection."],
    LMX_MODEL_UNAVAILABLE: ["此模型已不可用，请刷新目录并重新选择。", "The model is unavailable; refresh and choose again."],
    LMX_PROVIDER_COOLDOWN: ["此连接暂时冷却中，请稍后手动重试，无需更换模型。", "This connection is cooling down; retry manually later with the same model."],
    LMX_PROVIDER_UNAVAILABLE: ["上游连接失败或中断，请检查官方客户端与网络连接后手动重试。", "The upstream connection failed or disconnected; check the official client and network, then retry manually."],
    LMX_RATE_LIMITED: ["请求限流，请稍后手动重试。", "Rate limited; retry manually later."],
    LMX_QUOTA_EXHAUSTED: ["上游额度不足，请检查账户。", "Upstream quota is exhausted."],
    LMX_UNSUPPORTED_PARAMETER: ["上游不支持该参数或超出已报告范围，请恢复默认。", "The parameter is unsupported or out of range; restore defaults."],
    LMX_UNSUPPORTED_MESSAGES: ["此连接不支持当前消息角色或历史，请使用单轮模式。", "This connection cannot preserve these roles or history; use single-turn mode."],
    LMX_TIMEOUT: ["请求超时；部分回答保留，请手动重试。", "Request timed out; partial text is retained."],
    LMX_STREAM_INTERRUPTED: ["流式响应异常中断，部分回答不作为完整历史。", "The stream ended unexpectedly; partial text is excluded from history."],
    LMX_CHAT_BUSY: ["已有请求运行，请等待或停止。", "A request is running; wait or stop it."],
    LMX_CONTEXT_LIMIT: ["输入超过上游上下文限制，请缩短消息后手动重试。", "Input exceeds the upstream context limit; shorten the message before retrying."],
    LMX_OUTPUT_TRUNCATED: ["回答被上游截断；部分文字保留，不进入完整历史。", "The upstream truncated the answer; partial text is excluded from history."],
    LMX_REFUSAL: ["上游拒绝或过滤了回答；不会自动重试。", "The upstream refused or filtered the answer; no automatic retry."],
    LMX_OUTPUT_TOO_LARGE: ["回答超过 4 MiB 接收上限，已结束本机接收。", "The answer exceeded the 4 MiB reception limit."],
    LMX_EMPTY_RESPONSE: ["上游没有返回有效文字；不会自动重试。", "No usable text returned; no automatic retry."],
  };
  return messages[code]?.[zh ? 0 : 1] ?? (zh ? "请求失败，请检查服务或连接后手动重试。" : "Request failed; check the service and retry manually.");
}
