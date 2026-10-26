# GLM Coding Plan Provider

## Purpose

使用官方 Coding Plan 专用 OpenAI 兼容 API，属于 Provider Adapter，不依赖任何桌面 Agent。专用地址为 `https://open.bigmodel.cn/api/coding/paas/v4`，不能替换成普通计费地址。

## Supported OS

HTTPS Provider 组件可在 macOS/Windows 使用；当前只通过本地模拟 HTTP 服务验证。

## Detection

无本机应用扫描要求；必须由用户显式输入 Coding Plan Key 并确认套餐适用性。

## Supported versions

HTTP 协议按官方文档接口；未做真实账户验收。

## Connection/auth strategy

`Authorization: Bearer <Key>`。Key 只存在内存中，后续由 OS 凭据库持久化；当前 GUI 未开放连接入口，不读取其他工具配置中的密钥。

## Capabilities

共享 `HttpProvider` 支持 `/models` 解析、文本 `chat/completions` 非流式与 SSE；若专用套餐接口不提供 `/models`，返回 Unsupported，不伪造型号。

## Model discovery

仅消费接口返回的 `data[].id`，费用一律 `UNKNOWN`。

## Quota discovery

未接官方套餐余额接口；不可从本地 token 统计反推余额。

## Free-tier detection

未知，不进入 free 池。

## Session handling

Provider HTTP 调用无 Agent 原生会话；Phase 7 的本地 Conversation 处理跨轮上下文。

## Streaming format

SSE 仅提取 `choices[0].delta.content`，要求 `[DONE]` 完整结束；兼容 LF/CRLF 帧。

## Process lifecycle

无子进程；请求有超时，断开客户端后流被丢弃。

## Error mapping

401/403→认证失败，429→限流，5xx→Provider 不可用；不透传含敏感信息的原始响应。

## Security boundary

默认只允许 HTTPS（测试用 loopback HTTP），禁用 HTTP 重定向，Key 不进 SQLite/日志。官方 GLM Coding Plan 对可使用工具有限定，LocalAiMux 适用性仍须人工确认；当前组件不注册进生产 Gateway。

## Known limitations

用户未提供 Key，未完成套餐适用性、专用接口模型列表或真实收费边界验收；不能宣称真实可用。

## Fixtures

`adapter-sdk/tests/provider_http.rs` 使用合成 Key 与本地 HTTP 服务。

## Compatibility tests

`cargo test -p localmux-adapter-sdk -p localmux-adapter-glm`。

## Manual verification checklist

人工确认 LocalAiMux 属于套餐允许的接入方式 → 用户显式输入 Key → Keychain 存储 → 专用地址模型目录 → 小额请求与账单核对 → SSE/错误/撤销。
