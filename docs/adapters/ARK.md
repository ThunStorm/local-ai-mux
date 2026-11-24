# Ark Coding Plan Provider

## Purpose

通过官方方舟 Coding Plan 专用 OpenAI 兼容地址 `https://ark.cn-beijing.volces.com/api/coding/v3` 作为 Provider Adapter 接入。普通方舟地址 `/api/v3` 可能产生套餐外费用，代码固定不用它。

## Supported OS

HTTPS Provider 组件可在 macOS/Windows 使用；当前仅本地模拟 HTTP 验证。

## Detection

无桌面安装要求；用户必须显式提供 Coding Plan Key，并确认套餐和模型。

## Supported versions

按官方专用 API 文档实现；未完成真实账户验收。

## Connection/auth strategy

Bearer Key 只在内存中持有，Phase 8 由 OS 凭据库保存；不读取第三方工具配置。

## Capabilities

共享 `HttpProvider` 支持目录、文本非流式与 SSE。若套餐专用接口不支持 `/models`，不会返回硬编码模型名单。

## Model discovery

仅解析接口明确返回的模型 ID，费用 `UNKNOWN`。官方页面上的型号列表不等于用户账户实时可用目录。

## Quota discovery

未接套餐余额；本地统计不能当作官方余额。

## Free-tier detection

未知，不进入 free 池。

## Session handling

无 Agent 原生会话；Phase 7 管理本地 Conversation。

## Streaming format

SSE 文本 delta 与 `[DONE]`，支持 LF/CRLF 帧；半截响应报错。

## Process lifecycle

无子进程；HTTP 请求有限时，禁止重定向。

## Error mapping

401/403、429、5xx 转为统一错误，不透传原文。

## Security boundary

固定 Coding Plan 专用地址，仅 HTTPS；Key 不进 SQLite/日志。当前组件不注册生产 Gateway，避免未经账单核对就误用通用计费接口。

## Known limitations

无用户 Key、无真实套餐账单核对、未验证专用 `/models`；不能宣称已连接或免费。

## Fixtures

`adapter-sdk/tests/provider_http.rs` 的合成 Key 与本地服务器。

## Compatibility tests

`cargo test -p localmux-adapter-sdk -p localmux-adapter-ark`。

## Manual verification checklist

用户确认套餐和专用 Key → OS 凭据库存储 → 专用地址连接 → 官方模型目录/请求结果 → 账单归属核对 → SSE/撤销/额度状态。
