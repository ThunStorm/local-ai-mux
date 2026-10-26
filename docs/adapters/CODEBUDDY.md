# CodeBuddy Adapter

## Purpose

通过官方 [CodeBuddy Code CLI](https://www.codebuddy.cn/docs/cli/reference) 的 headless 入口接入。CodeBuddy CN 编辑器、WorkBuddy 桌面应用与独立 CLI 分开展示，任一桌面登录不自动等于 CLI 登录或可调用订阅。

## Supported OS

CLI 文档覆盖 macOS/Windows；2026-10-06 本机已安装独立 CodeBuddy Code CLI 2.161.0，入口为 `~/.nvm/versions/node/v24.18.0/bin/codebuddy`。WorkBuddy 包内虽有脚本但缺少完整运行文件，CodeBuddy CN.app 的 `app/bin/code` 是编辑器启动器，两者都不作为此 Adapter 的可连接 CLI。

## Detection

发现引擎检查 PATH、已知目录中的独立 `codebuddy`/`cbc` 可执行文件。WorkBuddy 和 CodeBuddy CN 只作桌面登记；未发现独立 CLI 时不显示模型连接按钮。

## Supported versions

本机 2.161.0 已通过公开版本、配置命令与只读模型目录检查；真实推理/登录归属尚未验收，界面仍标“版本未验证”。

## Connection/auth strategy

用户点击添加后，在隔离临时目录运行官方 `codebuddy config get model`，有明确模型 ID 时优先采用；未设置覆盖值（空输出/null）时，从公开 `codebuddy --help` 的 `--model <model>` 区段解析 `Currently supported: (...)` 中的 ID。无法明确解析则拒绝，不写死型号或代用户设置全局模型。CLI 使用自己的登录态，LocalAiMux 不读取 `~/.codebuddy/models.json`，因为它可能含明文 API Key。目录验证不代表账户登录/型号可用性已经通过。
本机 WorkBuddy 包内脚本实际需 Node ≥18.20.8，且缺少完整 CLI 运行文件；仅通过私有环境开关调用精简 bundle 不符合稳定公开接口要求。独立官方 CLI 运行时优先选择已安装的兼容 NVM Node，`config get model` 返回空值表示没有模型覆盖，并不等于 CLI 没有可用模型。应用内的桌面登录按钮只打开对应官方应用，不保证其账号与独立 CLI 共用。

## Capabilities

当前代码支持已选模型的文本调用、`stream-json` 增量与原生 session ID。工具和 MCP 禁用，权限固定 `dontAsk`，单轮上限为 1。

## Model discovery

模型映射为 `codebuddy/<id>`；优先读取当前选中模型，否则使用 CLI 公开帮助显式报告的模型 ID。本机 2.161.0 的只读 Adapter 检查得到 17 个 ID。这是安装 CLI 的能力目录，不能视作该账户的全部授权模型；费用/额度保持 UNKNOWN。帮助文案或格式变化时返回 `Unsupported`，不猜测。

## Quota discovery

未找到经过验证的 CLI 机器可读官方余额接口；保持未知，Phase 6 不会从请求统计推导剩余额度。

## Free-tier detection

全部模型为 `UNKNOWN`；模型名称、倍率或套餐不自动等于免费。

## Session handling

读取流式结果里的 `session_id/sessionId`，后续用 `--resume <id>`；本地持久映射已在 Phase 7 实现。

## Streaming format

`--include-partial-messages --output-format stream-json` 的 `text_delta` 进入 Gateway SSE；思考内容不外露。

## Process lifecycle

独立 CLI 子进程与临时目录，流结束或客户端断开时终止句柄；完整进程组清理在 Phase 8。

## Error mapping

认证、限流、Credits/Quota 与进程失败映射到统一错误，原始供应商输出不透传。

## Security boundary

提示词走 stdin；指定 `--tools ""`、`--strict-mcp-config`、`--permission-mode dontAsk`，不使用 `-y` 或 `CODEBUDDY_IS_SANDBOX`。不秘密读取供应商配置或凭据。

## Known limitations

独立 CodeBuddy Code CLI 已安装，目录检查通过；应用内显式添加、登录归属和真实推理仍待用户操作。WorkBuddy 与 CodeBuddy CN 的账号不能借此标为已接入；WorkBuddy 调研见 [独立记录](WORKBUDDY.md)。离线 fixture 不能替代真实验收。

## Fixtures

`crates/adapters/codebuddy/tests/fixtures/codebuddy.sh` 提供已选模型、两个增量与 session ID；`unset_model.sh` 覆盖未设置模型覆盖值时的公开帮助目录回退。

## Compatibility tests

`cargo test -p localmux-adapter-codebuddy`；显式指定 `LOCALMUX_CODEBUDDY_CLI` 后可执行 `cargo test -p localmux-adapter-codebuddy installed_cli_catalog -- --ignored --nocapture`，只查目录、不连接保存或推理。Gateway 三 Adapter 联合测试验证目录无 ID 冲突与精确路由。

## Manual verification checklist

安装官方 CLI → 用户登录 → LocalAiMux 点击添加 → 核对已选模型与账号 → 官方 OpenAI SDK 非流式/流式调用 → 会话续接与取消 → 验证无凭据泄漏。

## 2026-10-06 能力协商实施

前置 system 通过公开 --system-prompt，连续指令按顺序合并；developer/Chat history 拒绝。调用隔离 cwd、tools 空值、strict MCP 和 --setting-sources 空值，不修改供应商全局 model/settings。--json-schema 只登记接口候选，保证/Schema 子集/最终失败尚未核对，标准 JSON 格式继续明确不可用；没有公开输出 Token 控制，不使用 max-turns 或字节截断冒充。CLI 无可信请求用量/结束原因/实际动态型号时返回未知。 完整边界和证据见[实施验收](../architecture/CAPABILITY_NEGOTIATION_IMPLEMENTATION_2026-10-06.md)。
