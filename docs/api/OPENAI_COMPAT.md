# OpenAI 兼容 API

已实现 `GET /v1/models` 和 `POST /v1/chat/completions`，支持 Mock `mock/echo` 文字输入、非流式和 SSE；请求需要 `Authorization: Bearer lmx_...`。聊天只接受 `system/user/assistant` 文字消息，工具与多模态尚未启用。官方 Python/TypeScript SDK 已通过 Mock 模型列表、非流式与流式消费测试。

Phase 6 增加动态虚拟模型 `free`、`cheap`、`balanced`、`best`。只有存在符合条件的目标时才在模型列表展示。`free` 耗尽时返回 429 `LMX_FREE_POOL_EXHAUSTED`；真实 Adapter 目前均为费用 UNKNOWN，因此不会被自动放入免费池。`/v1/responses` 在 Phase 7 实现。
