# Kimi Code Adapter

## Purpose

检测 Kimi Code CLI 作为 Agent Runtime 的候选入口。官方 [Print 模式](https://moonshotai.github.io/kimi-code/en/reference/kimi-command) 提供 `-p` 与 `stream-json`，但当前实现不把它暴露给 Gateway。

## Supported OS

CLI 官方支持 macOS/Windows；本机未发现 Kimi CLI。

## Detection

发现引擎只检查可执行文件与应用安装元数据，不读取 Kimi 登录配置或令牌。

## Supported versions

无已验证版本；保持 `UNSUPPORTED`。

## Connection/auth strategy

未执行登录或连接。Kimi 的 Print 模式会自动处理 Agent 工具，当前无法在保留官方登录配置的同时证明工具完全禁用；不能把它作为默认 `INFERENCE_ONLY` 推理端点。

## Capabilities

本阶段只保留 Adapter 契约和显式 `Unsupported` 状态。模型、推理、流式和续接均未对外宣称可用。

## Model discovery

没有可靠的机器可读已登录模型目录，返回 `Unsupported`，不写死 Kimi 型号。

## Quota discovery

未知。

## Free-tier detection

未知，不进入 free 池。

## Session handling

官方 `--session` 支持续接，但本 Adapter 尚未启用。

## Streaming format

官方 `stream-json` 按整条 Assistant 消息输出，当前不保证 token 级增量；尚未接入。

## Process lifecycle

不启动真实 Kimi 子进程。

## Error mapping

检测到但未接入时返回 `LMX_ADAPTER_UNSUPPORTED_VERSION` 对应的统一错误。

## Security boundary

不读取供应商私有凭据，不使用 GUI 自动化，不以自动许可模式运行有工具能力的 Agent。

## Known limitations

需要找到经过官方文档确认的无工具推理配置并完成真实登录/调用验收，才能从 `UNSUPPORTED` 提升；目前 Phase 5 的 Kimi 真实验收未完成。

## Fixtures

单元测试断言不暴露模型；后续真实协议确定后再增加输出 fixture。

## Compatibility tests

`cargo test -p localmux-adapter-kimi`。

## Manual verification checklist

确认官方纯推理权限配置 → 用户显式连接 → 动态模型目录 → 非流式/流式/续接 → 取消和子进程清理 → 不泄漏凭据。
