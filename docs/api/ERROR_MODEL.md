# 错误模型

Gateway 返回 HTTP 状态与 OpenAI 风格 `{"error":{"message","type","code","error_code"}}`。已覆盖无 Key/权限不足/撤销、畸形请求、模型不可用、Provider 不可用、额度与限流、free 池耗尽、Session 未找到或无法续接。错误不包含供应商凭据、提示词或原始上游响应。

`LMX_SESSION_NOT_FOUND` 表示 `previous_response_id` 不存在、未完成或不是该会话最新响应；`LMX_SESSION_RESUME_FAILED` 表示上游不支持续接、用户要求切换模型或黏性 Provider 不可用。`LMX_FREE_POOL_EXHAUSTED` 永不触发收费模型静默回退。

`LMX_UNSUPPORTED_MESSAGES`（400）表示目标 Adapter 无法保证多角色或历史 Chat 消息的语义；`LMX_UNSUPPORTED_PARAMETER`（400）表示参数能力未知、不支持或超出实际型号报告的范围。客户端应恢复模型默认参数、改选有已报告能力的连接，或仅发送单条 user 文本。Gateway/Adapter 不静默丢弃字段。

桌面试对话只展示白名单错误码及本机中文说明；未知错误不把 Gateway/上游原文发给界面。`LMX_API_STOPPED`/`LMX_KEY_MISSING` 表示前置条件未就绪；`LMX_CHAT_BUSY` 表示已有请求；`LMX_TIMEOUT`/`LMX_STREAM_INTERRUPTED`/`LMX_EMPTY_RESPONSE`/`LMX_OUTPUT_TOO_LARGE` 表示接收失败，草稿与部分回答保留但不进入完成历史。无自动重试、换模型或收费回退。

2026-10-06：`LMX_PROVIDER_COOLDOWN`（503）表示同一连接已进入短期熔断，当前型号仍可能在目录中；Chat exact route 与 Responses 均保留这个区别，不误报 404 `LMX_MODEL_UNAVAILABLE`。用户稍后手动重试，无需删除连接或换模型。流式错误保留白名单鉴权/额度/限流/连接失败码；`LMX_PROCESS_TIMEOUT` 对应桌面的 `LMX_TIMEOUT`。厂商原文、HTTP 请求和认证资料不透传。

2026-10-06 能力协商：配置类 400 的 error 新增 param、reason_code、limiting_layer、retryable=false 与安全 request_id。`LMX_UNSUPPORTED_PARAMETER` 保留兼容码，reason_code 区分 conflict/out_of_range/not_implemented/unknown_constraints/guarantee_unverified 等，不吞字段；`LMX_UNSUPPORTED_MESSAGES` 指出 messages 与角色/历史映射层。能力限制不能归因到底层模型。`LMX_CONTEXT_LIMIT` 与 `LMX_REFUSAL` 独立报告上下文边界和拒绝，不触发暂时连接熔断。桌面 `LMX_OUTPUT_TRUNCATED` 表示收到 length，部分文字不进入完整历史。
