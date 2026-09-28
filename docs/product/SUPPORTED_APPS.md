# 应用支持矩阵

以下为截至 2026-09-28 的已实现能力；「离线」表示 fixture/Mock 测试，不代表用户订阅已通过真实调用。模型只展示上游实际报告的 ID。额度 UNKNOWN 表示没有可验证的机器数据。

| 应用 / Bridge | 发现 | 显式连接 | 模型目录 | 额度 | 文本调用 | 平台验收 |
|---|---|---|---|---|---|---|
| Codex / 官方 App Server | macOS、Windows 候选 | 代码及离线 fixture；真实调用待用户操作 | 官方 `model/list` 动态列表 | 官方主/次用量窗口分别保存，非 Credits 余额；本机此前已读到单窗口 | 单 user 文本及 SSE 离线通过；多消息明确拒绝 | macOS 安装版可发现；Windows CI 通过、实机待验 |
| Qoder / 官方 CLI | macOS、Windows 候选；本机已安装 CLI 1.1.64 | 代码及离线 fixture；真实登录待用户操作 | 当前 CLI JSON 契约未在真实账号验证，失败则不报模型 | UNKNOWN | 单 user 文本及 SSE 离线通过；多消息明确拒绝 | macOS 可发现；Windows 实机待验 |
| WorkBuddy / 包内 CodeBuddy CLI | macOS 包内脚本，版本 2.147.0 | 代码及离线 fixture；登录共享待验证 | 仅当前选中模型，PARTIAL | UNKNOWN | 单 user 文本及 SSE 离线通过；多消息明确拒绝 | macOS 可发现；Windows 实机待验 |
| MiMo Desktop | 可登记桌面应用 | 无已验证桌面订阅 Bridge | UNKNOWN | UNKNOWN | 不可路由 | macOS 登记已验证 |
| MiMo 开放平台 / API Key | 与桌面登记分离 | 用户另行提供 Key 后验证目录并存 OS 凭据库 | 官方 API 实时返回 | UNKNOWN | HTTP 文本通道离线测试 | 真实 Key 未提供 |
| Ollama、TRAE 等仅检测应用 | 可登记或检测 | 无当前可路由连接 | UNKNOWN | UNKNOWN | 不可路由 | 以发现页诊断为准 |

连接路径、安全边界及逐项人工验收见 [Codex](../adapters/CODEX.md)、[Qoder](../adapters/QODER.md)、[CodeBuddy](../adapters/CODEBUDDY.md)；实际进度见 [交接记录](../HANDOFF.md)。
