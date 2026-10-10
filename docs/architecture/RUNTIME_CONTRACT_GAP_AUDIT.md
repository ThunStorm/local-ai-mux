# Gateway 运行契约差距审计（2026-10-10 / M0）

基线 `codex/dev / d7a80b5`。仅本仓库源码/离线 fixture；无 Provider 生成、连接或权限变更。独立计划见 [实施计划](../LOCALAIMUX_GATEWAY_CAPABILITY_RELIABILITY_PLAN.md)。

| 范围 | 已有事实 | 本轮差距与文件 |
| --- | --- | --- |
| API | `/v1/models`、Chat、Responses 和 `/api/chat/models` v2 已存在；Key/Agent/连接权限过滤、严格未知字段、store:false 已实现 | 保持接口；gateway/lib.rs、responses.rs 增加入站关联和统一期限 |
| 时间 | 总期 600、流空闲 180、锁等待 30、启动间隔 5 秒；取锁和节流后才开始总期；目录/鉴权未纳入 | 请求入口单调预算；队列/间隔/启动/流/持久化分别受剩余预算约束，保留默认值 |
| 取消 | ActiveRun/OwnedMutexGuard 随 HTTP/SSE 生命周期释放；CLI kill_on_drop，Codex AppServer Drop 清理直接子进程 | 测试未消费 Body、部分接收断线、spawn/协议失败、子进程；远端停止始终 UNKNOWN，不盲重试 |
| 目录 | ChatCapabilities.details 已分层；版本/时间/live_verified；目录缓存 300 秒，失败移除 | 暴露刷新时间/到期/未知理由；fixture 不升格真实验证；沿用目录，不建第二套 registry |
| 响应 | typed delta/usage/completed、未知 null、length/filter/cancel、last vs total、前置指令规范化已有 | 锁定 actual_model/finish 来源、扩展长度预算与标准 SDK 故障回归 |
| 调用记录 | 0011 元数据与 0012 opt-in 正文；store:false 绕过历史/正文/映射/事件；终态写入先于成功帧 | 0013 可空有界诊断；排队/节流/Adapter/首字/总期/安全原因独立，旧行不回填；扩展既有记录详情 |
| Adapter | Codex AppServer、Qoder/CN 与 CodeBuddy 公共 CLI、隔离 cwd/禁用工具 | 离线进程和错误矩阵；不读隐藏 token/私有配置，不自动更新授权 |
| 验证 | verify.sh、全仓 Clippy、官方 Python/TS Mock smoke 已有 | 扩展现有 golden，提供命令、计数、平台与回滚证据；真实上游/Windows 原生 NOT_VERIFIED |

时间路径：HTTP middleware 入站 → auth → schema → CallGuard → route/catalog → Agent lane → interval → Adapter start → typed output → terminal persistence → response encoding/body。入站期限不延长；心跳不算上游输出。

合成基线（非安装版数据）：
```json
{"schema_version":2,"data":[{"id":"mock/echo","provider":"mock","adapter":"mock","identity_scope":"adapter_compat","chat":{"system":"SUPPORTED","history":"SUPPORTED","source":"mock fixture"}}]}
```
```json
{"status":"ok","active_tasks":0,"execution_limits":{"request_timeout_seconds":600,"stream_idle_timeout_seconds":180,"agent_max_concurrency":1,"agent_queue_timeout_seconds":30,"agent_request_interval_seconds":5}}
```
```json
{"model":"mock/echo","messages":[{"role":"system","content":"synthetic instruction"},{"role":"user","content":"synthetic text"}],"store":false}
```
用量/实际上游型号/结束原因/远端取消没有证据时保留 null/unknown。M7 明确独立选做，本轮不实施新 Adapter、多实例、工具/视觉/history 或真实 Schema 保证。M0 无迁移；回滚仅删除此审计。
