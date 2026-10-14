# LocalAiMux：前缀缓存与会话复用能力评估、最小可选改造计划

> 目标分支：`ThunStorm/local-ai-mux@codex/dev`。2026-10-14 静态基线与 M0/M1 实施；与至简业务实现解耦。**保持现有请求语义**。用户已在本轮明确授权开发验证、本机部署及提交推送。

## 一、改造前已证实的现状（静态基线）

1. `POST /v1/chat/completions` 为逐请求无状态入口：`crates/gateway/src/lib.rs:chat_inner` 接收每次完整 messages；Codex Adapter `crates/adapters/codex/src/lib.rs:prepare_turn` 对无 session 调用 `thread/start`，不自动继承前一条 Chat 业务指令。不能在至简第一片发送 System 后省略后续 System。
2. `POST /v1/responses` 支持 `previous_response_id`：`crates/gateway/src/responses.rs:context` 限定同 owner key、相同 sticky model/adapter、最新 Response、可用原生会话 ID 后续接。不是所有 Adapter 都有 resume；无法续接会拒绝，不悄悄跨 Provider 续接。
3. `ResponsesRequest` 仅有 model/input/stream/previous_response_id/store/metadata 等有限字段；目前输入仅接受单条纯文本 user，不可直接当作完整的 System/Developer 预热与显式 Prompt Cache 接口。
4. Codex Adapter 已解析 `thread/tokenUsage/updated` 的 `last.cachedInputTokens`，写入 `TokenUsage.cached_input_tokens`（`crates/adapters/codex/src/lib.rs:codex_usage`）；这只是上游报告的缓存数据，不是 Mux 自研 KV Cache。
5. Chat 标准 `usage` 当前只发送 `prompt_tokens/completion_tokens/total_tokens`，细分缓存位于扩展的 `localmux.usage.cached_input_tokens`；至简当前 `providers/llm.py:safe_runtime_metadata` 并不读取该嵌套值，所以业务端尚未可靠显示真实缓存效果。
6. LocalAiMux 的调用记录/正文保存（默认关闭）不是缓存复用；catalog cache 缓存模型目录，不是 Prompt 缓存。现有 ChatRequest `deny_unknown_fields` 没有 `prompt_cache_options` 等显式前缀缓存参数。不能直接把 OpenAI 官方 API 的 Prompt Caching 功能等同于订阅 CLI Adapter 的能力。

## 二、建议结论

**现阶段不必为至简专门重写 Mux**。优先改至简：保留每片明确系统规则、复用稳定前缀、减少真正重复上下文、正确计量 reported cached tokens。Mux 仅做通用、可复用的小型可选增强，不加「至简专用」业务端点或全局隐藏会话。

## 三、开发项（本轮实施 M0/M1；M2 继续延期）

### M0 P1：缓存命中用量标准化/诊断（小范围）

- 在统一响应映射层评估把上游 **validated** `cached_input_tokens` 投影到标准 OpenAI 兼容字段 `usage.prompt_tokens_details.cached_tokens`（仅报告可信时写入；值不存在保留 UNKNOWN，不填 0），保持 `localmux.usage` 现有扩展与老客户端兼容。仅处理已验证 `source=provider_reported` 和 `scope=request/last_turn` 的 token usage。
- 如果不改标准响应，可仅增强 `localmux.usage` 的文档和端到端对照验收；至简端从扩展字段中安全读取，效果相同。本轮选择标准字段作为通用消费契约，扩展字段兼容保留；两者必须来自同一校验快照。
- UI 记录详情可展示 `reported_input_tokens`、`reported_cached_input_tokens`、缓存占比（分母可信且非零时）、`usage_status`，独立于 Mux 的虚拟路由额度。
- 补充 codex fixture 和 `crates/gateway/tests/` 测试：有缓存/无缓存字段/未知用量/流式/非流式/last_turn 对比 session_total；缓存 Token 不能被加到 input 总量上。

### M1 P2：通用能力声明（不改现有路由）

- 在 `ChatCapabilities.details` 或新版本 capability 中声明 `session_resume` 和 `prompt_cache_control` 的受支持/不支持/未知与证据来源，分别按 API 接口、精确模型、Adapter 校验，不能因为 Codex 有 `cachedInputTokens` 就声称显式 Prompt Cache 可控。
- 若需面向业务端开放会话模式，新增普通公开能力门禁，不做「自动把同视频的 Chat 请求聚合成一个线程」的隐式功能；不得改变 `/v1/chat/completions` 的标准无状态语义。

### M2 P3：真实支持后才做的通用前缀缓存控制

- 只对明确支持且已完成真实试验的 HTTP Provider/接口/模型透传原生缓存参数；保持协议兼容、安全白名单和 strict schema，不转给不支持的 Codex/Qoder/CodeBuddy Adapter。
- 标识 cache write/read、prefix fingerprint、reported cached tokens、model/adapter/stage；**不落持久化 Prompt 全文或隐私内容**。
- 基于合成 fixture 做三类测试：相同静态前缀+不同 user 后缀、Prompt 版本变化、路由/模型切换。再选择少量真实调用验证，结果缺失记 UNKNOWN。

## 四、否决项与风险

- 不开发跨用户、跨 Provider 的泛化 Prompt/KV 状态共享缓存；容易混淆隔离、安全与行为语义。
- 不通过修改 Mux 自动删掉用户显式传递的 System/Developer Message。
- 不让 Responses 续接跨过未完成、截断、拒绝、取消或不同模型的 Response；保留现有拒绝边界。
- 不把“缓存命中可降低 API 计费”直接外推成“Codex 订阅限额同比例减少”。
- 不做依赖私有 CLI 接口、隐藏凭据、注入供应商未支持参数、提高账户能力的实现。

## 五、最小验收顺序

真实缓存效果的建议验收顺序：在用户单独明确授权后，抓取不含私密内容的现有 Mux 真实响应，确认当前安装 Codex 是否上报 cached_input_tokens，再验证业务端读取与显示。本轮已授权通用标准字段/能力目录开发，先独立完成 M0/M1 离线契约验收，不以前述真实采样作为已完成证据。M2 不纳入本轮范围。

相关入口：`crates/gateway/src/lib.rs`、`crates/gateway/src/responses.rs`、`crates/adapters/codex/src/lib.rs`、`crates/adapter-sdk/src/lib.rs`、`crates/api-types/src/lib.rs`、`docs/api/OPENAI_COMPAT.md`、`docs/architecture/SESSION_MODEL.md`；现有 `docs/HANDOFF.md` 为交接信息，不另建重复总纲。

本轮不修改至简仓库，不发起真实供应商生成或新连接，不做数据迁移。部署与推送已获本轮授权；真实缓存效果、订阅额度收益与 Windows 原生仍待独立验收。

## 六、2026-10-14 实施与验收

- M0：Chat 非流式与 `include_usage` SSE 在可信缓存值存在时添加 `usage.prompt_tokens_details.cached_tokens`；Responses 同步添加 `usage.input_tokens_details.cached_tokens`，流式位于终态 response。`localmux.usage.cached_input_tokens` 保留兼容，所有字段复用 `TokenUsage::validated`，仅接受 provider_reported + request/last_turn。输入计数缺失或缓存超过输入时仅缓存细分降未知；缺失不填 0，明确上报 0 则保留；input/total 原值不加缓存。非流式事件收集同样先校验再回用最后可信快照，累计会话终态不会覆盖本轮数据。
- M1：现有 v2 `chat.details` 增加 `prompt_cache_control`，补全 `session_resume`；各自 `api_support` 使用完整端点路径。Chat resume=UNSUPPORTED；Responses 按 Adapter 当前公开契约声明，仍须同 owner、最新成功响应、相同模型/Adapter 和真实 native session ID。Codex 核对当前公开 resume Schema 的 threadId，缺失/失败 UNKNOWN；Qoder/CN/CodeBuddy 核对当前公开 --resume，HTTP Chat 桥原生续接 UNSUPPORTED。证据带版本/来源/时效与 live_verified=false，过期的端点 SUPPORTED 同步降 UNKNOWN。缓存控制在两个入口均 UNSUPPORTED，限制层 gateway；底层模型能力保持 UNKNOWN，不由缓存计数推断可控。
- 保留严格请求 schema、完整消息转发、原权限/路由/会话边界。无缓存参数透传、隐式 Chat 会话或正文持久化改造；可选记录 UI/缓存占比不在最小交付范围，缓存诊断由标准响应和原 localmux 扩展提供，不回填历史。无需迁移，旧能力客户端可忽略 api_support。
- 离线入口：`crates/gateway/tests/prefix_cache.rs`（两协议×两种传输×九种用量情形及能力/strict schema），Codex 假 App Server 覆盖 last/total、缓存缺失/越界/未知、缺 threadId/导出失败与恢复；领域测试覆盖能力过期，SDK collector 覆盖累计终态回退。官方 Python/TS smoke 在隔离服务中核对标准与扩展缓存值相等、input/total 不变。真实 Provider 未调用，不能据此标记真实缓存命中。
- 字段依据：[OpenAI Chat Completions 参考](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create) 的 prompt_tokens_details.cached_tokens（2026-10-14 核对）；只借用标准用量形状，不继承其上游缓存控制能力。

最终验证与安装来源见 `docs/HANDOFF.md` 本轮记录。M2 继续以真实接口/型号支持及用户显式真实试验为前置；本文件第五节原建议的真实采样未执行，不作为 M0/M1 离线开发的已完成证据。
