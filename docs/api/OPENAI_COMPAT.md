# OpenAI 兼容 API

已实现 `GET /v1/models` 和 `POST /v1/chat/completions`，支持 Mock `mock/echo` 文字输入、非流式和 SSE；请求需要 `Authorization: Bearer lmx_...`。聊天只接受 `system/user/assistant` 文字消息，工具与多模态尚未启用。官方 Python/TypeScript SDK 已通过 Mock 模型列表、非流式与流式消费测试。

产品对齐阶段的 Chat 请求保留消息顺序与角色。HTTP Provider 将完整消息数组传给上游；Codex/Qoder/CodeBuddy 的现有单轮 CLI/App Server 桥只接受单条 user 文本，多角色或历史消息返回 HTTP 400 `LMX_UNSUPPORTED_MESSAGES`，不会静默丢弃。`temperature`、`max_tokens` 尚无统一透传契约，传入时返回 `LMX_UNSUPPORTED_PARAMETER`；未知 Chat 字段返回 400。Responses 当前只接受字符串或单条 user 文本输入，多条消息明确拒绝。此接口只承诺 OpenAI 兼容文本子集；Mock 的消息回显仅供离线测试，不代表真实上游验收。

Phase 6 增加动态虚拟模型 `free`、`cheap`、`balanced`、`best`。只有存在符合条件的目标时才在模型列表展示。`free` 耗尽时返回 429 `LMX_FREE_POOL_EXHAUSTED`；真实 Adapter 目前均为费用 UNKNOWN，因此不会被自动放入免费池。`/v1/responses` 在 Phase 7 实现。
