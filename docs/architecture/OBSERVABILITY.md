# Observability

Phase 0 initializes `tracing-subscriber` with `RUST_LOG` filtering. Do not log secrets, prompt text or response text. Structured request events arrive with the Gateway.

2026-10-08：生成调用通过独立 SQLite `call_records` 形成元数据历史，覆盖 Chat、Responses、试对话、短测试及失败/超时/取消/部分终态。默认记录元数据，Chat/Responses `store:false` 不保存。只收集可信 typed 用量/型号和本地安全码，默认不持久化正文、请求参数、metadata 或上游错误原文；用量和费用未知不能当 0。页面支持筛选、分页、详情、Responses 各轮关联和经确认清理终态记录，原路由事件保留。SQLite 失败的诊断文字固定，不包含数据内容。见[会话模型](SESSION_MODEL.md)。

2026-10-09：新增用户显式开启的对话正文录制，作用于独立 SQLite 表，原 tracing/诊断日志仍禁止正文。关闭立即停止后续正文写入，记录/备份中已有正文保留，列表仅包含元数据；正文详情经独立 IPC 读取并对常见凭据脱敏。语义与边界见[会话模型](SESSION_MODEL.md#对话正文录制2026-10-09)。

2026-10-10：原 call_records 添加有界可空诊断，不新建 telemetry 系统。排队、节流、本机 Adapter/首字/总耗时分开；来源及字节测量边界见[运行诊断](SESSION_MODEL.md#有界运行诊断2026-10-10--m3)。未知云端时间和远端取消不补 0/PASS；不改变默认正文关闭与 store:false。记录详情支持旧版本空值，连接状态只反映本机当前活动，不证明订阅余额或实际型号质量。
