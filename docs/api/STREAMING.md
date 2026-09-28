# 流式响应

Chat 使用 `text/event-stream`，发送角色、文本增量、`finish_reason=stop` 和 `data: [DONE]`。Mock 返回单个文本块；Codex Adapter 的 `item/agentMessage/delta` 会逐块映射为 SSE。首块与后续块有超时限制，客户端断开时流被丢弃，所拥有的子进程会终止。官方 Python/TypeScript SDK 已对 Mock 路径消费成功；Codex 真实订阅流仍待显式连接后验收。
