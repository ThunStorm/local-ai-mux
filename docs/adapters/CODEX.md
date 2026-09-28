# Codex Adapter

## Purpose

把用户显式添加的 Codex CLI 作为 Agent Runtime，通过官方 [Codex App Server](https://learn.chatgpt.com/docs/app-server) 的 stdio 协议接入。它使用 Codex 自己维护的登录状态；LocalMux 不读取或复制 ChatGPT Token。

## Supported OS

macOS 与 Windows 的可执行文件路径均可检测；本机仅在 macOS 上完成运行验证，Windows 仍需 CI/实机验证。Windows `.cmd` 包装脚本只显示检测结果，不作为可连接入口。

## Detection

扫描 PATH、常见安装目录及 macOS `ChatGPT.app/Contents/Resources/codex`。用户点击该 CLI 的「添加」后，后端再次扫描并验证路径，防止任意可执行文件注入。

## Supported versions

本机发现 `codex-cli 0.155.0-alpha.16.4`。尚无经过真实调用验收的 `max_verified_version`；界面应显示「版本未验证」，连接失败时明确报错，不能据此宣称正式支持。

## Connection/auth strategy

仅在用户点击「添加」后启动 `codex app-server`，完成 `initialize`/`initialized`，读取 `account/gatewayOAuth/read` 与 `account/read`，随后调用 `model/list`。不会触发登录流程；未登录时提示用户到 Codex 官方客户端登录后重试。SQLite 只保存已授权 CLI 路径，不保存凭据。

## Capabilities

当前代码支持模型发现、文本调用、事件增量输出、按原生 thread ID 续接和进程退出时中断。工具/图片等能力未向 OpenAI API 宣称。每次调用使用隔离临时目录、`readOnly` 沙箱及 `never` 审批。

## Model discovery

`model/list` 返回的可见模型动态映射为 `codex/<model>`；不写死型号。未知或缺失字段会保守处理。账户与模型请求失败时不展示伪造模型。

## Quota discovery

使用公开 App Server `account/rateLimits/read`，优先读取 `rateLimitsByLimitId.codex`，缺失时读兼容字段 `rateLimits`。只有 `ordinaryUsageAllowed=true` 且窗口 `usedPercent` 合法时，上报剩余百分比、窗口时长和重置时间；这不是账户 Credits 余额。缺失或后端未确认可用时保持未知。

## Free-tier detection

全部模型标记 `UNKNOWN`，不因订阅存在而推断免费。

## Session handling

新调用走 `thread/start`，续接走 `thread/resume`；返回原生 thread ID，Phase 7 将建立持久化 Conversation 映射。

## Streaming format

解析 `item/agentMessage/delta`、`item/completed` 和 `turn/completed`；增量文本进入 Gateway SSE，`[DONE]` 由 Gateway 结束。上游异常映射为统一错误。

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
