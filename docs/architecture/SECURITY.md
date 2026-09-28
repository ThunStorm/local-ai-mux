# 安全边界（Phase 8）

Gateway 只绑定 `127.0.0.1:8317`。`/api/health` 只提供存活状态；模型目录、Chat 和 Responses 分别要求 `models:read`、`chat:invoke`、`responses:invoke`。API Key 只在创建时显示一次，SQLite 仅保存 SHA-256 哈希与权限元数据；撤销后后续请求立即失效。每个 Key 可限制模型/虚拟路由、连接和每分钟 1–600 次请求。默认 `INFERENCE_ONLY` 阻止 Agent Adapter；用户在 GUI 显式选 `READ_ONLY_AGENT` 时才授予 `agent:invoke`。`WORKSPACE_WRITE` 和 `FULL_AGENT` 暂不接受，避免声称尚不能执行的沙箱约束。

当前 Codex/Qoder/CodeBuddy Adapter 向官方运行时请求只读或禁用工具模式。LocalMux 无法证明所有上游版本都严格遵守；GUI 明示由上游运行时管理工具权限。虚拟路由命中目标后仍再次核验 Key 的路由、连接、执行权限。Responses 会话绑定创建时的 Key ID，其他 Key 即使持有响应 ID 也不能续接；升级前无法确定归属的旧会话仍可在 GUI 查看，但不可通过 API 续接。请求体最大 1 MiB，失败返回结构化错误，不把提示词、回答或原始上游响应写入 SQLite/日志。

`localmux-secrets` 使用系统凭据库：macOS Keychain、Windows Credential Manager。SQLite `credential_refs` 只容纳引用元数据，不保存密钥值。该接口尚未连接 Phase 5 的 GLM/Ark 真实 Provider；需要用户在 GUI 显式添加且先完成资费与真实接口验收。日志输出层统一遮盖 Authorization、Bearer、LocalMux Key、常见 token/cookie/password 字段和邮箱；诊断导出辅助函数额外遮盖用户目录路径。Tauri 生产内容安全策略限制资源来源。

验证边界：离线安全测试覆盖默认拒绝 Agent、显式权限、路由/连接限制、限流、超大请求和撤销；系统凭据库的真实写入/读取及 Windows 原生运行仍需在对应平台验收。
