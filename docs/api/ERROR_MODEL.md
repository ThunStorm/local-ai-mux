# 错误模型

v0.2.0：HTTP/自定义Adapter非流式大结果现在与流式一致受8MiB正文上限约束，超限返回502 LMX_OUTPUT_TOO_LARGE；HTTP目录JSON4MiB、响应JSON64MiB也在解析前限制，不把资源限制误报为型号不存在或厂商停服。

v0.2.0：30–1200秒CLI范围由后端校验，Gateway总预算1200；LMX_PROCESS_TIMEOUT和固定timeout_layer/timeout_limit_ms形状兼容。超过范围不落设置、不发生成；旧版合法设置保留。startup/RPC/HTTP/client等独立层不因CLI范围扩大而改写，详见[Streaming当前契约](STREAMING.md#v020-当前计时契约2026-10-26)。

Gateway 返回 HTTP 状态与 OpenAI 风格 `{"error":{"message","type","code","error_code"}}`。已覆盖无 Key/权限不足/撤销、畸形请求、模型不可用、Provider 不可用、额度与限流、free 池耗尽、Session 未找到或无法续接。错误不包含供应商凭据、提示词或原始上游响应。

`LMX_SESSION_NOT_FOUND` 表示 `previous_response_id` 不存在、未完成或不是该会话最新响应；`LMX_SESSION_RESUME_FAILED` 表示上游不支持续接、用户要求切换模型或黏性 Provider 不可用。`LMX_FREE_POOL_EXHAUSTED` 永不触发收费模型静默回退。

`LMX_UNSUPPORTED_MESSAGES`（400）表示目标 Adapter 无法保证多角色或历史 Chat 消息的语义；`LMX_UNSUPPORTED_PARAMETER`（400）表示参数能力未知、不支持或超出实际型号报告的范围。客户端应恢复模型默认参数、改选有已报告能力的连接，或仅发送单条 user 文本。Gateway/Adapter 不静默丢弃字段。

桌面试对话只展示白名单错误码及本机中文说明；未知错误不把 Gateway/上游原文发给界面。`LMX_API_STOPPED`/`LMX_KEY_MISSING` 表示前置条件未就绪；`LMX_CHAT_BUSY` 表示已有请求；`LMX_TIMEOUT`/`LMX_STREAM_INTERRUPTED`/`LMX_EMPTY_RESPONSE`/`LMX_OUTPUT_TOO_LARGE` 表示接收失败，草稿与部分回答保留但不进入完成历史。无自动重试、换模型或收费回退。

2026-10-06：`LMX_PROVIDER_COOLDOWN`（503）表示同一连接已进入短期熔断，当前型号仍可能在目录中；Chat exact route 与 Responses 均保留这个区别，不误报 404 `LMX_MODEL_UNAVAILABLE`。用户稍后手动重试，无需删除连接或换模型。流式错误保留白名单鉴权/额度/限流/连接失败码；`LMX_PROCESS_TIMEOUT` 对应桌面的 `LMX_TIMEOUT`。厂商原文、HTTP 请求和认证资料不透传。

2026-10-06 能力协商：配置类 400 的 error 新增 param、reason_code、limiting_layer、retryable=false 与安全 request_id。`LMX_UNSUPPORTED_PARAMETER` 保留兼容码，reason_code 区分 conflict/out_of_range/not_implemented/unknown_constraints/guarantee_unverified 等，不吞字段；`LMX_UNSUPPORTED_MESSAGES` 指出 messages 与角色/历史映射层。能力限制不能归因到底层模型。`LMX_CONTEXT_LIMIT` 与 `LMX_REFUSAL` 独立报告上下文边界和拒绝，不触发暂时连接熔断。桌面 `LMX_OUTPUT_TRUNCATED` 表示收到 length，部分文字不进入完整历史。


2026-10-08：Gateway 总期与 CLI 空闲超时统一返回 504 LMX_PROCESS_TIMEOUT；连接队列等待超过 30 秒返回 429 LMX_RATE_LIMITED、Retry-After=5，表示本机忙，不等于厂商额度耗尽。CLI result 的 is_error/subtype=error 在内存识别 quota/credit、rate limit、authentication 后返回白名单类别，不保存原始错误结果。LMX_QUOTA_EXHAUSTED、LMX_AUTH_REQUIRED 和 LMX_RATE_LIMITED 保持区别。所有错误释放占用，未完成/超时不保存成功响应映射；仅关闭本机进程句柄不证明上游停止计费。

2026-10-10 M1：HTTP X-Request-ID 与非流错误 error.request_id 使用同一 Gateway 入站 ID，客户端头不能覆盖。总预算从入站起算；排队/节流耗尽总预算为 504 LMX_PROCESS_TIMEOUT，单独 lane 的 30 秒上限仍为 429 LMX_RATE_LIMITED / Retry-After=5。

2026-10-10 M4：配置类错误的 param/reason_code/limiting_layer 经过固定词表过滤，未知 Adapter 自由文本降为安全占位；稳定 LMX_* 码不变，厂商错误原文不透传。Chat SSE 与 Responses 失败事件同样带本次入站 request_id；HTTP 200 后的 SSE 错误以协议错误帧表示，不把其 HTTP 状态改写为事后 504。


2026-10-21：CLI/Gateway 超时保留 LMX_PROCESS_TIMEOUT，新增固定词表 timeout_layer 和实际 timeout_limit_ms；未知来源不再默认归为 overall_deadline。Chat/Responses、流/非流及新调用诊断保持同源，具体层和边界见 [CLI 限时](STREAMING.md#cli-空闲限时与超时归因2026-10-21)。


## 首包超时与连接冷却

Qoder CLI 明确报告首包超时时，返回 504 `LMX_PROCESS_TIMEOUT`，原因 `upstream_first_payload_timeout`，层 `upstream_first_payload`，时限以 `timeout_limit_ms` 表示。SSE 已开始时在失败帧携带同一组字段。无法确认的 CLI 错误保留通用分类，上游执行与用量保持未知。

连接冷却返回 503 `LMX_PROVIDER_COOLDOWN`，HTTP `Retry-After` 为实际剩余时间向上取整的秒数。错误体的 `upstream_dispatched=false` 表示本次未进入 Adapter 推理，`cooldown_reason` 区分 `transient_failure`、`rate_limited`、`quota_exhausted`。调用方可据此安排恢复，Gateway 不自动重发。

### CLI stderr 诊断补充

真实视频重跑中，Qoder 内部会话记录了首包超时，但公开接口返回通用 502。此前 CLI 流式调用丢弃 stderr，无法使用该通道上可能存在的明确错误。Qoder 路径现在异步排空 stderr，仅在内存保留最近 8 KiB。若 CLI 结束失败且未产出正文，按固定首包签名识别，未知错误仍返回通用 502。空的 JSON error 字段不再中止正常结果。成功请求、重试策略与上游调用次数均沿用原路径。子进程夹具验证 stdout、stderr 与空 error 字段，真实样本能否由此获得明确归因待观察。
