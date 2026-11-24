# Observability

v0.2.0：health新增service=LocalAiMux与workspace编译version；入站总期1200秒、CLI/Agent空闲30–1200秒。已有安全request_id、实际触发timeout_layer/timeout_limit_ms、记录快照和未知来源保留；不把当前设置写回旧诊断，不记录原始Provider错误或凭据。当前验证见[封版](../releases/V0_2_0_FREEZE.md)。

Phase 0 initializes `tracing-subscriber` with `RUST_LOG` filtering. Do not log secrets, prompt text or response text. Structured request events arrive with the Gateway.

2026-10-08：生成调用通过独立 SQLite `call_records` 形成元数据历史，覆盖 Chat、Responses、试对话、短测试及失败/超时/取消/部分终态。默认记录元数据，Chat/Responses `store:false` 不保存。只收集可信 typed 用量/型号和本地安全码，默认不持久化正文、请求参数、metadata 或上游错误原文；用量和费用未知不能当 0。页面支持筛选、分页、详情、Responses 各轮关联和经确认清理终态记录，原路由事件保留。SQLite 失败的诊断文字固定，不包含数据内容。见[会话模型](SESSION_MODEL.md)。

2026-10-09：新增用户显式开启的对话正文录制，作用于独立 SQLite 表，原 tracing/诊断日志仍禁止正文。关闭立即停止后续正文写入，记录/备份中已有正文保留，列表仅包含元数据；正文详情经独立 IPC 读取并对常见凭据脱敏。语义与边界见[会话模型](SESSION_MODEL.md#对话正文录制2026-10-09)。

2026-10-10：原 call_records 添加有界可空诊断，不新建 telemetry 系统。排队、节流、本机 Adapter/首字/总耗时分开；来源及字节测量边界见[运行诊断](SESSION_MODEL.md#有界运行诊断2026-10-10--m3)。未知云端时间和远端取消不补 0/PASS；不改变默认正文关闭与 store:false。记录详情支持旧版本空值，连接状态只反映本机当前活动，不证明订阅余额或实际型号质量。

## 文件日志与占用（2026-10-26）

桌面启动在当前业务数据目录的`logs/`写入脱敏tracing诊断，stderr保留；正文/Prompt/原始上游错误仍不入日志。活动文件`localmux.log`最大5MiB，单事件最多64KiB；达到大小或跨UTC日轮转`localmux-<UTC日秒数>-<序号>.log.gz`。默认14天、100MiB，偏好可选1–90天/10–1024MiB、下次启动生效。启动、跨日/大小轮转及每分钟维护；先删超龄与最老归档，预留一个活动文件空间，文件0600/目录0700。仅处理本应用严格命名的常规日志文件，不遍历链接、不删SQLite/正文/人工备份。文件写入/维护失败不阻断生成，偏好页报状态。占用为逻辑字节，分数据库（含WAL/SHM）、人工备份、日志和固定Key目录；不读Key内容。启动、每小时和手动刷新采样，至少一小时才能显示最近30天数据库净增与日折算，可负、未知不补零，不作为写入吞吐/未来预测。详见[数据治理](DATA_LIFECYCLE.md)。

文件初始化写入固定、不含用户数据的标记，以区分可写日志与尚无事件；事件仍遵守RUST_LOG筛选。新增订阅器→脱敏→文件链路回归，避免仅以目录存在判断文件日志有效。
