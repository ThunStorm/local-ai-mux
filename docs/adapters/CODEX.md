# Codex Adapter

## Purpose

把用户显式添加的 Codex CLI 作为 Agent Runtime，通过官方 [Codex App Server](https://learn.chatgpt.com/docs/app-server) 的 stdio 协议接入。它使用 Codex 自己维护的登录状态；LocalMux 不读取或复制 ChatGPT Token。

## Supported OS

macOS 与 Windows 的可执行文件路径均可检测；本机仅在 macOS 上完成运行验证，Windows 仍需 CI/实机验证。Windows `.cmd` 包装脚本只显示检测结果，不作为可连接入口。

## Detection

扫描 PATH、常见安装目录及 macOS `ChatGPT.app/Contents/Resources/codex`。用户点击该 CLI 的「添加」后，后端再次扫描并验证路径，防止任意可执行文件注入。

## Supported versions

2026-10-14：能力判断改为当前安装 CLI 的公开 Schema 自动协商，取消版本白名单。每次连接/目录刷新/调用预检执行 `codex app-server generate-json-schema --out <隔离临时目录>`（不启用 experimental 字段），按 v2/ThreadStartParams、ThreadResumeParams 中可保留任意字符串的 baseInstructions/developerInstructions 独立声明 system/developer；两种线程操作均须支持。turn/start.effort 支持公开字符串/本地引用/可空类型/枚举时，与精确 model/list 档位取交集；默认档位不在有效列表则省略。新版本无需重新发布 LocalAiMux 来更新版本白名单。

导出复用 owned 进程、有界 stdout/stderr 和10秒期限，只读取三个普通文件、每个最多1MiB；临时产物随调用清理。缺失、失败、损坏、循环引用或未映射约束保持 UNKNOWN，details.unknown_reason 区分 public_schema_unavailable / public_schema_field_unverified，证据来源为 installed_public_schema_and_adapter_mapping，live_verified 仍 false。无持久协议缓存或“默认全部支持”；单 user 默认调用省略未核对指令字段，有前置指令时明确拒绝，不降级成 user。历史/工具/图像、输出上限与 JSON 保证不因此开放。

复用 Gateway 五分钟目录刷新；每次实际调用另核对当前 CLI，因此原路径升级/导出恢复会重新识别。至简等持久缓存接口能力的消费端仍需重新读取并保存快照；LocalAiMux 不跨应用静默修改配置或伪造业务探测证据。当前本机 CLI 为 0.162.0-alpha.17.2，真实生成验收仍独立；0.160.0 的两个实际型号单轮流式结果见交接，界面保留「版本未验证」。无需重新连接、扩权或重建 Key。

## Connection/auth strategy

仅在用户点击「添加」后启动 `codex app-server`，完成 `initialize`/`initialized`，读取 `account/gatewayOAuth/read` 与 `account/read`，随后调用 `model/list`。不会触发登录流程；未登录时提示用户到 Codex 官方客户端登录后重试。SQLite 只保存已授权 CLI 路径，不保存凭据。

## Capabilities

当前代码支持模型发现、文本调用、事件增量输出、按原生 thread ID 续接和进程退出时中断。工具/图片等能力未向 OpenAI API 宣称。每次调用使用隔离临时目录、`readOnly` 沙箱及 `never` 审批。

## Model discovery

`model/list` 返回的可见模型动态映射为 `codex/<model>`；不写死型号。未知或缺失字段会保守处理。账户与模型请求失败时不展示伪造模型。

## Quota discovery

使用公开 App Server `account/rateLimits/read`，优先读取 `rateLimitsByLimitId.codex`，缺失时读兼容字段 `rateLimits`。只有 `ordinaryUsageAllowed=true` 且窗口 `usedPercent` 合法时，分别上报 primary 与 secondary 的剩余百分比、窗口时长和重置时间；旧单快照接口只取首个有效窗口以兼容旧路由。这不是账户 Credits 余额，两个窗口不相加。缺失或后端未确认可用时保持未知，旧观测超时后标为过期。

## Free-tier detection

全部模型标记 `UNKNOWN`，不因订阅存在而推断免费。

## Session handling

新调用走 `thread/start`，续接走 `thread/resume`；两者的 `sandbox` 是字符串 `read-only`。`turn/start.sandboxPolicy.type` 则是 `readOnly`，两个字段不能共用同一拼写。返回原生 thread ID，Phase 7 已建立持久化 Conversation 映射。

## Streaming format

解析 `item/agentMessage/delta`、`item/completed` 和 `turn/completed`；增量文本进入 Gateway SSE，`[DONE]` 由 Gateway 结束。官方 `error` 通知与 failed turn 按 `codexErrorInfo` 映射认证、额度、限流或连接失败；首个错误即结束本次进程，厂商原文不透传，避免隐藏重试和空等超时。

## Process lifecycle

每个请求拥有独立 App Server 子进程，drop 时杀进程。此实现隔离取消与故障，但吞吐量受启动成本限制；达到真实使用量后再考虑复用进程。长时间无事件按超时处理。

## Error mapping

登录、限流、额度、模型不可用、未支持方法和进程失败映射到 `AdapterError`。供应商原始错误文案不透传给 API 客户端。

## Security boundary

不秘密读取 Keychain、Codex 配置或令牌；不自动执行登录。CLI 仅能从扫描结果中选择，推理固定只读沙箱和隔离工作目录。供应商 stdout 按 JSON 解析，stderr 不作为模型输出。

## Known limitations

本机 alpha CLI 的真实请求与额度读取需以用户已显式连接的账户复核；离线 fixture 与编译通过不等于生产可用。当前没有工具调用、账号切换、完整进程组清理，也未验证 Windows `.exe` 的真实连接。

## Fixtures

`crates/adapters/codex/tests/fixtures/` 包含版本、模型目录、过期认证、限流错误与 App Server 回合/增量事件样例。

## Compatibility tests

`cargo test -p localmux-adapter-codex` 覆盖动态模型、UNKNOWN 分类、鉴权/限流错误、线程创建、续接与流式输出；Gateway 测试验证两个增量块分别进入 SSE。官方 Python/TypeScript SDK 已对 Mock Gateway 重跑通过。

## Manual verification checklist

1. 在桌面发现页确认 Codex CLI 路径和「版本未验证」提示。
2. 用户点击「添加」，检查官方登录状态及模型目录；未登录时不保存连接。
3. 创建 LocalMux Key、启动 API，通过官方 OpenAI SDK 调用 `codex/<真实返回模型>` 的非流式和流式接口。
4. 检查续接、取消、退出后进程清理，以及 SQLite/日志不含明文凭据。

## 2026-10-06 故障修复验收

用户报告 `codex/gpt-6-luna` 流式试对话约 0.2 秒失败。本机官方 App Server 握手、OAuth 门禁和 `model/list` 均正常，目录包含 `gpt-6-luna`；旧 `thread/start` 请求返回 `-32600`，指出 `readOnly` 不在 `read-only/workspace-write/danger-full-access` 枚举中。修正后，同一 CLI 的只读线程创建成功；未发送真实 `turn/start`，未消耗模型推理额度。离线 fixture 现强制检查 start/resume 的沙箱拼写，覆盖文本和 SSE。契约依据为本机 `codex app-server generate-json-schema` 与[官方 App Server 文档](https://learn.chatgpt.com/docs/app-server)。安装后的真实回答仍需用户在试对话中显式发送验收。

## GUI 启动与系统代理

2026-10-06 故障续查：线程与 turn 能在约 0.2 秒启动，但未带代理的实际短请求在 47 秒出现 `responseStreamDisconnected`；本机直连 ChatGPT 的无凭据检查超时，现有系统 HTTPS 代理可完成 TLS 并收到 HTTP 响应。macOS Adapter 在没有显式 HTTP(S)/ALL_PROXY 环境变量时，读取公开 `/usr/sbin/scutil --proxy` 的固定 HTTP/HTTPS 配置，仅向该 App Server 子进程注入代理；不修改系统/供应商配置，不读取代理凭据，保留已有 NO_PROXY 并补入 loopback。显式环境（包括空值）优先，PAC/自动代理和 SOCKS-only 不猜测；Windows 仍沿用已有环境。启用代理不等于真实模型成功，最终以安装版 Computer Use 验收记录为准。

## 安装版真实单轮流式验收

2026-10-06 按用户明确要求使用 Computer Use：在安装版 API 页恢复已授权只读 Agent 范围、刷新本机 Key、启动 API，试对话选择实际目录型号并发送同一短问题。`codex/gpt-6-luna` 完成 8.4 秒，`codex/gpt-6-sol` 完成 38.2 秒；Sol 的接收中、最终完成与两者默认参数/未携带历史由原生界面复核，结束后活动任务 0。CLI 0.160.0 的这两条真实流式链路通过；不以回答中的模型自述认证底层版本。其他型号、非流式/续接、远端取消/计费和 Windows 尚未因此验收。构建与数据库证据见 [交接](../HANDOFF.md)。

## 2026-10-06 能力协商实施

保留 model/list 的精确 inputModalities、defaultReasoningEffort、supportedReasoningEfforts；当前 0.160.0 的前置 system/developer 分别映射 baseInstructions/developerInstructions，effort 按枚举提交。公开 config/read 只提取 MCP 名称，请求级禁用外部执行/工具配置，隔离 cwd、never/read-only 保留；未发真实 turn 验证这些新控制。last usage 作为单 turn 快照，total 不累加；thread/model reroute 已报告型号才返回，completed 无结束原因时仍 unknown。Schema 和硬输出上限未开放；两种指令语义与安全控制的本轮验收仅为公开契约与 fixture。 完整边界和证据见[实施验收](../architecture/CAPABILITY_NEGOTIATION_IMPLEMENTATION_2026-10-06.md)。
