# Adapter 契约

`localmux-adapter-sdk::Adapter` 提供 `id/kind/health/list_models/get_quotas/invoke/invoke_stream`，输入与输出分别是 `NormalizedRequest`、`NormalizedResponse` 与增量 `NormalizedStream`。旧 `get_quota` 由默认 `get_quotas` 包装，Codex 分别报告多个窗口。真实 Agent/Provider 实现必须返回可验证模型能力与统一 `AdapterError`，不能在 Router/GUI 散布 Provider 分支，也不能把供应商异常或凭据原文直接发给 API 消费者。

Phase 1 的 `MockEcho` 只用于离线验收，生产桌面 Gateway 不预置 Mock。Codex 使用官方 App Server；Qoder 与 CodeBuddy 使用各自官方 headless CLI，并复用 `adapter-sdk::cli` 的 JSONL 进程管理。桌面控制层用静态 `ADAPTERS` 注册表统一 CLI 文件名和构造函数；每个已显式添加的 Adapter 才注册进 Gateway 动态目录。费用状态没有上游证据时一律 `UNKNOWN`。本机已安装 Qoder CLI、WorkBuddy 包内有 CodeBuddy CLI；离线协议测试仍不代表真实连接验收。

MiMo 开放平台走用户显式提供的 API Key 和固定 HTTPS 官方端点，使用 `HttpProvider`；MiMo Desktop 的安装登记与此连接无关。macOS WorkBuddy 包内的 CodeBuddy CLI 已加入发现，仍需验证其 Node.js 运行时和登录状态。没有可调用 CLI 的 Qoder 桌面包不会被直接注册。

`NormalizedRequest` 同时保留 Responses 单文本 `prompt` 和 Chat 的有序 `messages`。Adapter 必须使用消息数组或在发起上游调用前通过 `single_user_prompt()` 拒绝未支持的角色/历史；不能忽略 `messages` 后返回成功。HTTP Provider 发送完整消息数组，现有三种 Agent 桥仅保证单条 user 文本。
