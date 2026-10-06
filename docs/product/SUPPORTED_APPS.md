# 应用支持矩阵

以下为截至 2026-10-06 的已实现能力；「离线」表示 fixture/Mock 测试，不代表用户订阅已通过真实调用。模型只展示上游实际报告的名称或 ID。额度 UNKNOWN 表示没有可验证的机器数据。

| 应用 / Bridge | 发现 | 显式连接 | 模型目录 | 额度 | 文本调用 | 平台验收 |
|---|---|---|---|---|---|---|
| Codex / 官方 App Server | macOS、Windows 候选 | 用户已保存连接，当前 CLI 0.160.0 | 官方 `model/list` 动态列表 | 官方主/次用量窗口分别保存，非 Credits 余额；本机此前已读到单窗口 | 安装版 Computer Use 单 user SSE：gpt-6-luna 8.4 秒、gpt-6-sol 38.2 秒完成；多消息明确拒绝 | macOS 两型号真实流式通过；其他型号/续接/Windows 实机待验 |
| Qoder / 官方 CLI | Qoder.app 与全局 CLI 1.1.64 | 应用内按钮启动官方登录；本机已登录 | `MODEL` 文本目录实测报告 17 个名称 | UNKNOWN | 单 user 文本及 SSE 离线通过；多消息明确拒绝 | macOS 安装版已保存连接并显示 17 项，推理与 Windows 实机待验 |
| Qoder CN / 官方独立 CLI | Qoder CN.app、`~/.qoder-cn/entry/qodercn` 1.1.64 | 本机 CLI 未登录；应用内仅在认证缺失时引导官方登录 | 登录后由独立 `qodercn --list-models` 报告，不继承全球版 17 项 | UNKNOWN | 单 user 文本及 SSE 共用离线协议，真实调用待验 | macOS CLI 已安装，登录/模型/推理待验 |
| WorkBuddy | 本机独立 WorkBuddy.app 5.7.6 | 包内 2.156.0 CLI 的公开 config 命令缺完整产物；未证明桌面订阅可公开导出 | UNKNOWN | UNKNOWN | 不可路由 | 只读扫描可见，不进入应用主列表；见 [调研](../adapters/WORKBUDDY.md) |
| CodeBuddy CN | 独立 CodeBuddy CN.app | `app/bin/code` 为编辑器启动器 | UNKNOWN | UNKNOWN | 不可路由 | 只读扫描可见，不进入应用主列表 |
| CodeBuddy Code / 官方独立 CLI | 本机 2.161.0，NVM Node 24 安装；可扫描 `codebuddy`/`cbc` | 用户点击添加才保存连接；目录不证明登录/订阅 | 优先显式选中模型；未设置时解析公开帮助；本机报 17 ID，PARTIAL | UNKNOWN | 单 user 文本及 SSE 离线通过；多消息明确拒绝 | 真实 CLI 只读目录通过，真实推理与 Windows 实机待验 |
| MiMo Desktop | 可登记桌面应用 | 无已验证桌面订阅 Bridge；应用内可打开官方桌面登录 | UNKNOWN | UNKNOWN | 不可路由 | macOS 登记已验证 |
| MiMo Code CLI / 官方本机能力 API | 本机 `~/.mimocode/bin/mimo` 已发现 | 用户点击后启动 `mimo serve` 并签发限 MiMo 型号、最长 30 分钟的本机令牌 | 本机回环 `/v1/models` 在临时探测中返回 7 个型号；应用安装版连接待验 | UNKNOWN | HTTP 文本子集，真实推理未验 | macOS CLI 目录与限型号令牌已实测；与 Desktop 登录态不混同 |
| MiMo 开放平台 / API Key | 与桌面登记分离 | 用户另行提供 Key 后验证目录，本次运行仅存内存 | 官方 API 实时返回 | UNKNOWN | HTTP 文本通道离线测试 | 真实 Key 未提供 |
| Ollama、TRAE 等仅检测应用 | 可登记或检测 | 无当前可路由连接 | UNKNOWN | UNKNOWN | 不可路由 | 以发现页诊断为准 |

连接路径、安全边界及逐项人工验收见 [Codex](../adapters/CODEX.md)、[Qoder](../adapters/QODER.md)、[CodeBuddy](../adapters/CODEBUDDY.md)；实际进度见 [交接记录](../HANDOFF.md)。
