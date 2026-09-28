# 会话模型

LocalMux Conversation 与 Provider 原生 session 分开存储。`conversations` 保存本地 ID、实际模型、Adapter ID 和时间；`provider_sessions` 保存供应商 session ID 与续接能力；`responses` 把本地 `resp_` ID 映射到 Conversation；`run_events` 记录路由拒绝/不可用等不含提示词的事件。SQLite 不持久化提示词或回答正文。

新 Responses 请求建立 Conversation；如果 Adapter 返回原生 session ID，下一轮 `previous_response_id` 会固定原模型和 Adapter，并调用 `resume`/`resume_stream`。仅允许从最新完成的 Response 接续，避免把旧分支误接到已前进的供应商会话。供应商未提供原生 session ID 时明确拒绝续接。

如果黏性模型健康或费用分类变化导致虚拟路由失效，返回 `LMX_SESSION_RESUME_FAILED` 或 `LMX_FREE_POOL_EXHAUSTED`，并写入审计事件。没有完整上下文时不自动跨 Provider 回退。
