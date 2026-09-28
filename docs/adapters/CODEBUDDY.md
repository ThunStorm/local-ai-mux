# CodeBuddy Adapter

## Purpose

通过官方 [CodeBuddy Code CLI](https://www.codebuddy.cn/docs/cli/cli-reference) 的 headless 入口接入。CodeBuddy/WorkBuddy 桌面安装不自动等于 CLI 登录或可调用订阅。

## Supported OS

CLI 文档覆盖 macOS/Windows；本机 WorkBuddy 桌面包内有 `codebuddy` CLI 脚本（版本 2.147.0），已加入发现。它需要 Node.js 18.20.8 或以上；桌面登录是否与此 CLI 共用仍需用户在应用内显式连接验证。

## Detection

发现引擎检查 PATH、已知目录及 macOS WorkBuddy 包内的官方 `codebuddy` CLI。只有 `codebuddy`/`codebuddy.exe` CLI 可点击添加；桌面应用本身仍显示“仅检测到”。

## Supported versions

尚无已验证 CLI 版本；界面标“版本未验证”。

## Connection/auth strategy

用户点击添加后运行官方 `codebuddy config get model`，只接受单个明确模型 ID；CLI 使用自己的登录态，LocalMux 不读取 `~/.codebuddy/models.json`，因为它可能含明文 API Key。

## Capabilities

当前代码支持已选模型的文本调用、`stream-json` 增量与原生 session ID。工具和 MCP 禁用，权限固定 `dontAsk`，单轮上限为 1。

## Model discovery

目前只能动态读取 CLI 当前所选模型，映射为 `codebuddy/<id>`；没有官方稳定的完整机器可读模型目录时，不伪造其他模型。若 `config get model` 输出无法明确解析，返回 `Unsupported`。

## Quota discovery

未找到经过验证的 CLI 机器可读官方余额接口；保持未知，Phase 6 不会从请求统计推导剩余额度。

## Free-tier detection

全部模型为 `UNKNOWN`；模型名称、倍率或套餐不自动等于免费。

## Session handling

读取流式结果里的 `session_id/sessionId`，后续用 `--resume <id>`；本地持久映射在 Phase 7 补齐。

## Streaming format

`--include-partial-messages --output-format stream-json` 的 `text_delta` 进入 Gateway SSE；思考内容不外露。

## Process lifecycle

独立 CLI 子进程与临时目录，流结束或客户端断开时终止句柄；完整进程组清理在 Phase 8。

## Error mapping

认证、限流、Credits/Quota 与进程失败映射到统一错误，原始供应商输出不透传。

## Security boundary

提示词走 stdin；指定 `--tools ""`、`--strict-mcp-config`、`--permission-mode dontAsk`，不使用 `-y` 或 `CODEBUDDY_IS_SANDBOX`。不秘密读取供应商配置或凭据。

## Known limitations

本机没有 CodeBuddy CLI；模型目录当前仅有已选模型，不能满足完整列表要求。WorkBuddy 桌面订阅是否共享 CLI 登录仍未验证。离线 fixture 不能替代真实三上游验收。

## Fixtures

`crates/adapters/codebuddy/tests/fixtures/codebuddy.sh` 提供已选模型、两个增量与 session ID。

## Compatibility tests

`cargo test -p localmux-adapter-codebuddy`；Gateway 三 Adapter 联合测试验证目录无 ID 冲突与精确路由。

## Manual verification checklist

安装官方 CLI → 用户登录 → LocalMux 点击添加 → 核对已选模型与账号 → 官方 OpenAI SDK 非流式/流式调用 → 会话续接与取消 → 验证无凭据泄漏。
