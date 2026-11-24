# Responses API

Phase 7 已实现 `POST /v1/responses` 的核心文字输入、非流式响应、SSE 事件及 `previous_response_id`。请求需要含 `responses:invoke` scope 的 LocalAiMux API Key。输入可以是字符串或只含 `input_text` 的 user 消息；图片、工具、音频等未启用，传入会返回明确请求错误。

响应包含 `resp_` ID、完成状态、实际模型、Assistant `output_text` Item、`previous_response_id` 和 metadata。SSE 按 `response.created` → Item/Content added → `response.output_text.delta` → done → `response.completed` 发送；上游失败时发 `response.failed`，不写成可续接的完成响应。官方 Python/TypeScript OpenAI SDK 已通过 Mock 非流式和流式消费测试。

本地只保存 Response ID、Conversation ID、实际模型和上游 session ID。继续调用必须引用同一 Conversation 的**最新**已完成 Response，使用同一模型；支持原生续接的 Codex/Qoder/CodeBuddy 走供应商 session。Mock 不支持原生续接，引用它的 `previous_response_id` 会返回 `LMX_SESSION_RESUME_FAILED`，不会假装保留上下文。`store:false` 不保存本地映射，后续无法用该 ID 续接。

跨 Provider 自动续接默认关闭：LocalAiMux 没有保存完整提示词/回答，不能把缺失上下文暗中发给另一家。路由变化或黏性目标不可用会返回错误并记录不含正文的审计事件。
