# Observability

Phase 0 initializes `tracing-subscriber` with `RUST_LOG` filtering. Do not log secrets, prompt text or response text. Structured request events arrive with the Gateway.

2026-10-08：生成调用通过独立 SQLite `call_records` 形成元数据历史，覆盖 Chat、Responses、试对话、短测试及失败/超时/取消/部分终态。默认记录元数据，Chat/Responses `store:false` 不保存。只收集可信 typed 用量/型号和本地安全码，默认不持久化正文、请求参数、metadata 或上游错误原文；用量和费用未知不能当 0。页面支持筛选、分页、详情、Responses 各轮关联和经确认清理终态记录，原路由事件保留。SQLite 失败的诊断文字固定，不包含数据内容。见[会话模型](SESSION_MODEL.md)。

2026-10-09：新增用户显式开启的对话正文录制，作用于独立 SQLite 表，原 tracing/诊断日志仍禁止正文。关闭立即停止后续正文写入，记录/备份中已有正文保留，列表仅包含元数据；正文详情经独立 IPC 读取并对常见凭据脱敏。语义与边界见[会话模型](SESSION_MODEL.md#对话正文录制2026-10-09)。
