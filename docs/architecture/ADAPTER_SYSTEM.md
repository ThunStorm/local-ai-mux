# Adapter 契约

`localmux-adapter-sdk::Adapter` 提供 `id/kind/health/list_models/invoke/invoke_stream`，输入与输出分别是 `NormalizedRequest`、`NormalizedResponse` 与增量 `NormalizedStream`。真实 Agent/Provider 实现必须返回可验证模型能力与统一 `AdapterError`，不能在 Router/GUI 散布 Provider 分支，也不能把供应商异常或凭据原文直接发给 API 消费者。

Phase 1 的 `MockEcho` 只用于离线验收。Codex 使用官方 App Server；Qoder 与 CodeBuddy 使用各自官方 headless CLI，并复用 `adapter-sdk::cli` 的 JSONL 进程管理。每个已显式添加的 Adapter 注册进 Gateway 的动态目录。费用状态没有上游证据时一律 `UNKNOWN`。本机尚无独立 Qoder CLI；CodeBuddy CLI 虽在 WorkBuddy 包内，但离线协议测试不代表真实连接验收。

MiMo 开放平台走用户显式提供的 API Key 和固定 HTTPS 官方端点，使用 `HttpProvider`；MiMo Desktop 的安装登记与此连接无关。macOS WorkBuddy 包内的 CodeBuddy CLI 已加入发现，仍需验证其 Node.js 运行时和登录状态。没有可调用 CLI 的 Qoder 桌面包不会被直接注册。
