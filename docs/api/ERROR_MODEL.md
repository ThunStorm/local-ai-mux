# 错误模型

Gateway 返回 HTTP 状态与 OpenAI 风格 `{"error":{"message","type","code","error_code"}}`。已覆盖无 Key/权限不足/撤销、畸形请求、模型不可用、Provider 不可用、额度与限流、free 池耗尽、Session 未找到或无法续接。错误不包含供应商凭据、提示词或原始上游响应。

`LMX_SESSION_NOT_FOUND` 表示 `previous_response_id` 不存在、未完成或不是该会话最新响应；`LMX_SESSION_RESUME_FAILED` 表示上游不支持续接、用户要求切换模型或黏性 Provider 不可用。`LMX_FREE_POOL_EXHAUSTED` 永不触发收费模型静默回退。

`LMX_UNSUPPORTED_MESSAGES`（400）表示目标 Adapter 无法保证多角色或历史 Chat 消息的语义；`LMX_UNSUPPORTED_PARAMETER`（400）表示提交了当前文本子集未实现的采样/长度参数。客户端应改选支持完整消息的连接，或仅发送单条 user 文本。
