# Qoder Adapter

## Purpose

通过官方 [Qoder CLI headless](https://docs.qoder.com/cli/run-in-scripts) 接口接入用户自己的 Qoder 订阅。Qoder 桌面应用被检测到不等于 CLI 可用；只有用户显式添加已扫描到的 `qoder` CLI 后才启动运行时。

## Supported OS

CLI 官方支持 macOS/Windows；本机已安装 Qoder CLI 1.1.64，登录后的模型目录命令实测报告 17 个名称；真实推理仍未验收。Windows Adapter 已编译检查，仍需实机调用。

## Detection

发现引擎检查 PATH、常见安装位置及 macOS 官方安装器的 `~/.qoder/entry`。只有文件名为 `qoder`/`qoder.exe` 的扫描结果显示可连接；`com.qoder.app` 与 `com.qodercn.app` 是独立桌面应用，本机全局 CLI 只归 Qoder，不归 Qoder CN。

## Supported versions

本机 CLI `--version` 为 1.1.64，登录后目录已验证，推理未验证；界面标“版本未验证”，连接失败时不宣称支持。

## Connection/auth strategy

用户点击添加后执行官方 `qoder --list-models --output-format json`。CLI 使用它自己的登录状态，LocalAiMux 不读取 Qoder 配置或 Token；输出不是可解析目录时返回 `UNSUPPORTED`，不会造一个默认模型。
本机 1.1.64 在未登录时实际输出 `Not logged in`；现将该情况映射为 `AuthRequired`。应用内「登录 Qoder」仅由用户点击启动官方 `qoder login`，浏览器授权完成后需再点添加/刷新模型；登录命令的输出不进入日志。

## Capabilities

当前代码支持动态模型目录、headless 文本调用、`stream-json` 文本增量与会话 ID 映射。禁用内置工具、MCP，固定 `dont_ask` 权限与单轮上限。

## Model discovery

接受 `--list-models` 返回的 JSON 明确 ID，也接受本机 1.1.64 实测的 `MODEL` 表头、逐行模型名格式；只映射明确且合法的名称为 `qoder/<name>`。即使带 `--output-format json`，本机命令仍返回该文本表。遇到其他格式明确报不支持，不硬编码或推断 Qoder CN 的目录。

## Quota discovery

现有 Adapter 尚未实现机器读取余额，仍返回未知。2026-10-16 复核：官方 Agent SDK 已公开 `getUsageInfo()` 账户余额、Assistant 请求积分与 Result 会话累计积分；本机 CLI 1.1.64 的兼容性及真实采集未验收，详见[积分调研](CREDITS_RESEARCH_2026-10-16.md)。不会从本地用量反推官方剩余额度。

## Free-tier detection

全部模型为 `UNKNOWN`；“Efficient”低倍率不等于免费。

## Session handling

从 `stream-json` 的 `session_id/sessionId` 保存原生 ID，续接时传 `--session-id`。本地 Conversation 持久映射在 Phase 7 建立。

## Streaming format

只转发 `stream_event.event.delta.type=text_delta` 的文本；思考与工具参数不外露。完成时读取 `result`；若没有增量则输出完整结果一次。

## Process lifecycle

每次请求独立启动 CLI，并在请求中断时终止子进程。完整进程组清理属于 Phase 8。

## Error mapping

认证、速率、Credits/Quota 和进程失败统一映射为 `AdapterError`；无法解析的目录为 `Unsupported`。不会向 API 客户端透传原始供应商输出。

## Security boundary

提示词通过 stdin 发送，不出现在进程参数中。CLI 在独立临时目录执行，指定 `--tools ""`、`--strict-mcp-config`、`--permission-mode dont_ask`；不使用 `--yolo`。

## Known limitations

本机已验证 Qoder CLI 登录后的目录格式；这不证明 Qoder CN 共享登录，也不证明目录型号已通过推理。不能把桌面安装或离线 fixture 记为真实 Provider 调用验收。

## Fixtures

`crates/adapters/qoder/tests/fixtures/qoder.sh` 提供模型目录、两个文本增量和 session ID 的离线输出。

## Compatibility tests

`cargo test -p localmux-adapter-qoder`；Gateway 的三 Adapter 测试检查合并目录与精确路由。

## Manual verification checklist

安装官方 CLI → 用户登录 → LocalAiMux 点击添加 → 核对真实模型目录 → 官方 OpenAI SDK 调用非流式与流式 → 续接/取消/退出进程检查 → 额度来源核对。

## 2026-10-06 能力协商实施

前置 system 通过公开 --system-prompt 映射，连续同角色指令保持顺序；全球/CN 路径与目录独立。developer 和 Chat history 均拒绝。help/version 核对配置来源与工具控制，调用固定 --setting-sources 空值、tools 空值、strict MCP、单 turn。--max-output-tokens 只登记公开候选，精确型号上限/计数口径/SDK 配对未验证，两个标准输出上限均不可用。CLI 未取得可信单请求用量/结束原因/实际动态型号时保持 null/unknown；不能从 result 成功推断 stop。 完整边界和证据见[实施验收](../architecture/CAPABILITY_NEGOTIATION_IMPLEMENTATION_2026-10-06.md)。
