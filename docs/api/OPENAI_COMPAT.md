# OpenAI 兼容 API

已实现 `GET /v1/models` 和 `POST /v1/chat/completions`，支持 Mock `mock/echo` 文字输入、非流式和 SSE；请求需要 `Authorization: Bearer lmx_...`。聊天只接受 `system/user/assistant` 文字消息，工具与多模态尚未启用。官方 Python/TypeScript SDK 已通过 Mock 模型列表、非流式与流式消费测试。

Chat 保留消息顺序与角色。HTTP Provider 发送完整合法文字消息；Codex/Qoder/Qoder CN/CodeBuddy 的最小无状态桥支持连续前置 system + 单条 user。连续同角色指令按原顺序以明确边界合并，返回 `localmux.normalization=leading_instructions_v1`；Codex 按当前 CLI 的公开 Schema 自动核对 start/resume，分别以独立字段映射前置 system/developer，不按版本号猜兼容性；CLI 单 system 层拒绝 developer，不能降级成 user。非前置指令、未实现历史/工具/多模态、developer 后再放 system 都明确拒绝，不丢上下文。HTTP 的任意历史与 native session resume 是独立能力。

可选 temperature（0–2）、top_p（0–1）、max_tokens/max_completion_tokens（正整数）必须符合所选精确型号的有效能力和范围；未给字段不发送，f64 保留小数值。两个输出上限同时给出返回参数冲突，计数口径未核对时不互相冒充等价别名。reasoning_effort 仅按该型号合法枚举传递；Codex 自动核对当前公开 Schema 的 turn/start.effort，与精确 model/list 档位取交集，low 不等于 off。response_format 接受 text/json_object/json_schema 的标准形状，但首批只启用 text；Schema 保证未验证时明确报 not_implemented，不用 Prompt 伪装原生 JSON。Qoder/CN 输出限额/思考精确约束及 CodeBuddy/Codex 硬输出上限继续不可用。未支持、未知约束、冲突、范围错误均为 400，error 带 param/reason_code/limiting_layer/retryable=false/request_id；没有厂商原始认证信息或错误正文。其他未知字段不透传。

桌面及一般客户端共用 `GET /api/chat/models`：`{schema_version:2,data:[{id,provider,stream,chat,connection_id,adapter,identity_scope}]}`，保持 models:read、Agent/Key/连接过滤；不含虚拟路由或凭据。旧 chat 字段继续表示有效能力，details 分别报告 effective/native/public_interface/adapter_state、限制层、版本时间证据、约束和 live_verified。缺失数字/枚举不伪造；Codex 的原生 image 元数据不开放 Gateway 图片。连接身份目前仍是 adapter_compat，实例迁移独立实施。`/v1/models` 的 OpenAI 形状不变。

Chat 非流式与 SSE 同享可信元数据：未知 usage=null、finish_reason=null，localmux 显式记录 unknown；上游报告的 input/output/reasoning/cache、口径、实际型号与 resolved_model/request_id 分别保留。Codex 只使用 last turn 快照，不重加 session total；Qoder/CodeBuddy 累积用量不当成本次用量。标准 `stream_options:{include_usage:true}` 在流结束时增加 choices=[] 的 usage 块，未开启时保持旧流形状。截断映射 length，拒绝/过滤映射 content_filter，不能固定写成 stop；接收关闭只保证本机停止接收。Responses 仍仅接受字符串或单条 user 文本输入及已有 native resume；未知字段现在也拒绝，元数据/截断与 Chat 同义。

详细实现、测试及未验收边界见[能力协商实施](../architecture/CAPABILITY_NEGOTIATION_IMPLEMENTATION_2026-10-06.md)。

Phase 6 增加动态虚拟模型 `free`、`cheap`、`balanced`、`best`。只有存在符合条件的目标时才在模型列表展示。`free` 耗尽时返回 429 `LMX_FREE_POOL_EXHAUSTED`；真实 Adapter 目前均为费用 UNKNOWN，因此不会被自动放入免费池。`/v1/responses` 在 Phase 7 实现。


2026-10-08：GET /api/health 新增只读 execution_limits，报告 request_timeout_seconds=600、stream_idle_timeout_seconds=180、agent_max_concurrency=1、agent_queue_timeout_seconds=30、agent_request_interval_seconds=5；不含凭据或套餐余额。非流式与流式期限、连接并发及边界见[Streaming](STREAMING.md#2026-10-08-cli-调用期限与并发修订)。这些是本机保护参数，不是厂商限额，也不代表 600 秒桌面 playground 交互。

2026-10-08 调用记录：Chat 增加标准可选 `store:boolean`，与 Responses 一样默认 true；false 不持久化本次成功/失败调用元数据或 Responses 映射/路由事件，不改变上游请求/路由策略。认证成功且请求符合 schema 后创建元数据记录，后续参数/能力/目标权限拒绝也记录；认证或 schema 失败不登记。元数据保存与 native resume 独立，Chat 不创建可续接 Responses，默认不存正文；2026-10-09 起可由桌面开关显式录制。桌面 IPC 查询记录；本轮不新增外部 HTTP 历史查询或跨 Key 历史访问 API。详情见[扩展计划](../CALL_RECORDS_EXPANSION_PLAN_2026-10-08.md)。

2026-10-09：正文录制是本机全局 opt-in，默认关闭、跨重启保存。开启时保存新调用完整 messages/input 和 Gateway 收到的输出，包含部分失败结果；关闭后不继续记录在途输出，再开启不续录旧调用。标准 store:false 仍优先禁止一切本次保存，开关不发起上游请求、不改变 wire payload 或 native resume。没有新增外部历史/正文查询和开关 HTTP API。详情见[正文录制](../architecture/SESSION_MODEL.md#对话正文录制2026-10-09)。

2026-10-10 M2：能力目录仍为 schema_version=2，兼容添加 catalog 的 observed_at/expires_at/ttl_seconds/status/source/live_verified，以及 details 的 freshness/unknown_reason 和 evidence.expires_at。目录和当前版本能力观察最多五分钟有效；过期/无日期证据不能继续广告为 SUPPORTED，参数与角色有效字段同步降为 UNKNOWN，型号本身保留；刷新失败不复用旧真值。刷新只证明公开目录/契约，不证明真实生成，live_verified 不自动升级。缺字段的旧客户端继续使用原结构；不新增强制请求参数或业务专用字段。

2026-10-10 M4：标准 model 始终表示 Gateway 解析并选定的公开型号；localmux.resolved_model 同义，actual_model 仅取有来源且有界的上游元数据，不读答案自述。actual_model_source 为 provider_reported/app_server_thread/app_server_rerouted；finish_source 只有实际已知上游原因才为 provider_reported，其他为 null。usage 只接受 provider_reported 的 request/last_turn 快照，累计 session_total 不当本次使用，unknown=null。Typed Adapter 的无来源 stop/usage/model 会降未知，需显式提供来源。

localmux 采用固定键集合，JSON 不超过 4096 字节：原 finish_reason/usage/actual_model/normalization/usage_status/cancellation_guarantee 保留，兼容添加 source、response_id、timing 和 upstream_cancel_verified。request_id 与 HTTP X-Request-ID、调用记录关联 ID 一致；Chat/Responses 标准 id 与扩展 response_id 仍表示响应 ID，避免把请求关联误作续接 ID。timing 是可空本机测量快照，语义与记录诊断相同，不包含 Prompt/RPC/连接凭据。SSE 与非流统一来源与未知值；Responses 在整体 Body deadline 到达时发送 response.failed，序号继续本次流，不能发送逾期 completed。没有新增强制请求头或字段。

M2/M4 门禁补强：catalog.observed_at 在实际刷新时保存，读取不随系统时钟变化重新推算。最终 completed 的 usage 先按来源/口径校验，再采用最近一次可信 request/last_turn 快照；累计/未知来源不能抹掉已报告本次用量。终态记录、扩展与标准 usage 保持一致，没有可信快照时均为 null。
