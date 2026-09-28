# Qoder Adapter

## Purpose

通过官方 [Qoder CLI headless](https://docs.qoder.com/cli/run-in-scripts) 接口接入用户自己的 Qoder 订阅。Qoder 桌面应用被检测到不等于 CLI 可用；只有用户显式添加已扫描到的 `qoder` CLI 后才启动运行时。

## Supported OS

CLI 官方支持 macOS/Windows；本机仅安装 Qoder 桌面应用，CLI 真实运行未验收。Windows Adapter 已编译检查，仍需实机调用。

## Detection

发现引擎检查 PATH 和常见安装位置。只有文件名为 `qoder`/`qoder.exe` 的扫描结果显示可连接；桌面 `.app` 只显示“仅检测到”。

## Supported versions

本机没有 CLI，无法记录已验证版本；界面标“版本未验证”，连接失败时不宣称支持。

## Connection/auth strategy

用户点击添加后执行官方 `qoder --list-models --output-format json`。CLI 使用它自己的登录状态，LocalMux 不读取 Qoder 配置或 Token；输出不是可解析目录时返回 `UNSUPPORTED`，不会造一个默认模型。

## Capabilities

当前代码支持动态模型目录、headless 文本调用、`stream-json` 文本增量与会话 ID 映射。禁用内置工具、MCP，固定 `dont_ask` 权限与单轮上限。

## Model discovery

只接受 `--list-models` 返回的 JSON 数组或 `models/data` 数组中的明确 `id/model` 字段，映射为 `qoder/<id>`。官方文档尚未给出该命令的稳定 JSON schema，实际 CLI 可能不兼容；遇到其他格式明确报不支持。

## Quota discovery

官方 `/usage` 是交互面板，本阶段没有稳定可解析的剩余额度接口。Phase 6 不会从本地用量伪造官方剩余量。

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

本机未安装 Qoder CLI；尚未验证官方 `--list-models --output-format json` 的真实格式与 Qoder Global/CN 登录共享情况。不能把桌面安装或离线 fixture 记为真实 Provider 验收。

## Fixtures

`crates/adapters/qoder/tests/fixtures/qoder.sh` 提供模型目录、两个文本增量和 session ID 的离线输出。

## Compatibility tests

`cargo test -p localmux-adapter-qoder`；Gateway 的三 Adapter 测试检查合并目录与精确路由。

## Manual verification checklist

安装官方 CLI → 用户登录 → LocalMux 点击添加 → 核对真实模型目录 → 官方 OpenAI SDK 调用非流式与流式 → 续接/取消/退出进程检查 → 额度来源核对。
