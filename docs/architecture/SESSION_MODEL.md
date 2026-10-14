# 会话模型

LocalMux Conversation 与 Provider 原生 session 分开存储。`conversations` 保存本地 ID、实际模型、Adapter ID 和时间；`provider_sessions` 保存供应商 session ID 与续接能力；`responses` 把本地 `resp_` ID 映射到 Conversation；`run_events` 记录路由拒绝/不可用等不含提示词的事件。默认只保存元数据；从 2026-10-09 起，用户可显式开启正文录制，详见下文。

新 Responses 请求建立 Conversation；如果 Adapter 返回原生 session ID，下一轮 `previous_response_id` 会固定原模型和 Adapter，并调用 `resume`/`resume_stream`。仅允许从最新完成的 Response 接续，避免把旧分支误接到已前进的供应商会话。供应商未提供原生 session ID 时明确拒绝续接。

如果黏性模型健康或费用分类变化导致虚拟路由失效，返回 `LMX_SESSION_RESUME_FAILED` 或 `LMX_FREE_POOL_EXHAUSTED`，并写入审计事件。没有完整上下文时不自动跨 Provider 回退。

## 调用记录（2026-10-08）

[扩展计划与验收](../CALL_RECORDS_EXPANSION_PLAN_2026-10-08.md)新增独立 `call_records`，与成功 Responses 的续接映射分开。认证通过、请求符合 JSON schema 后登记 `in_progress`；参数/能力/目标权限/路由拒绝也可见。无效认证、解析失败、超大请求不写生成历史。Chat、Responses（流/非流）、桌面试对话及显式模型短测试均覆盖；Chat/Responses 的 `store:false` 同时关闭成功/失败调用记录和本次 Responses 映射/路由事件保存。没有旧调用回填。

终态：`completed`、`failed`、`timeout`、`cancelled`、`truncated`、`filtered`、`interrupted`。typed completed 确认接收完成，但 finish=unknown 仍原样显示 unknown；length/过滤/取消不存成功 Responses 映射。EOF 缺少 completed、流读取错误记 interrupted；HTTP/SSE future 丢弃记 cancelled，只承诺停止本机接收；若上游完整结果已接收，终态保留该已知结果。异常退出遗留 in_progress 在下次打开本机数据库时恢复为 interrupted，耗时无法恢复则保持 null。取消的异步补写若在退出前未完成，同样由启动恢复处理。

已知终态在发送完成帧前持久化，SDK 在 completed/[DONE] 后停止消费也能立即查询记录；终态写入失败不会发送成功完成帧，补写保留失败状态。

字段仅含本地调用/响应/会话 ID、所有者 Key ID、来源、接口、请求/解析/上游报告型号、Adapter、流标记、时间、本机耗时、安全错误码、结束原因及已报告 input/output/total tokens。请求参数、客户端 metadata、凭据和上游错误原文不入元数据表；正文仅在下述开关开启时保存在独立表；超长/无效型号及本机 Key、常见凭据形态的误填值仅保存无效标记；用量未知为 null，费用不推断。记录开始失败返回内部错误并阻止生成；普通 HTTP 终态写入失败返回内部错误，SSE 终态写入失败以传输错误报告；补写失败仅输出固定诊断文字，遗留行待启动恢复。

记录页使用自增 sequence 的稳定游标，每页 50 条，额外查询一条判断下一页；支持来源、状态、型号关键词与精确 conversation_id。Chat 不按 Key/型号猜测归组，Responses 可按同一 conversation_id 查看各轮。单条或全部终态记录可经确认删除，运行中记录保留；清理记录不删除 conversations/provider_sessions/responses/run_events，不撤销续接能力。旧会话入口继续展示原映射及路由审计事件。无自动保留期；正文开关从 2026-10-09 起按下节实施。

`x-localmux-source: playground` 仅是桌面 loopback 的来源标签，外部客户端可模仿，不作为权限证据；授权/Key 隔离仍由原有 Gateway 策略执行。短测试来源固定由内部入口设置。数据库启动恢复按当前单实例桌面生命周期执行，不作为多进程共享数据库协调协议。


## 对话正文录制（2026-10-09）

“记录对话内容”默认关闭，选择保存在 SQLite 并跨重启保留。关闭时原元数据记录继续存在；开启后，新调用在入口处保存完整 Chat messages（含 system/developer/user/assistant 与请求携带的历史）或 Responses input，保存 Gateway 实际收到的模型回答。显式短测试同样覆盖。开关不发起生成，也不改变原消息转发、能力/权限或费用策略。`store:false` 优先关闭本次元数据、正文与 Responses 映射保存。

`call_recording_settings` 保存 enabled 与 generation；`call_contents` 按 call_id 保存 request_json、response_text、录制代次和状态，外键级联清理。输入先落库；流式每个被接受的文字 delta 在向客户端转发前以完整脱敏快照落库，正常/失败/超时/截断/过滤/取消均保留已收到的内容。完整终态仍在完成帧前提交；客户端取消的元数据补写继续异步，重启把遗留正文标为 partial。已录内容不会自动注入后续上下文或用于跨 Provider 续接。

关闭在一个事务中关闭开关、递增代次并将正在录制的正文标为 stopped；所有输出写入都以 enabled、原代次及 recording 状态作原子条件。关闭提交后的增量、尚未返回的非流式结果不会继续写入；重新开启只影响新调用，不续录旧调用，已保存的输入/回答保留。调用开始时未开启的不做中途补录，旧元数据记录不回填。状态为 recording / complete / partial / stopped，反映正文捕获的范围，不代替元数据结果与上游取消保证。

正文中的普通邮箱、路径、换行与 Unicode 保留；LocalMux Key、常见 sk- Key、Bearer 及命名 token/password/cookie 字段脱敏，覆盖跨增量的凭据与结构化/camelCase 字段。仅正文副本脱敏，实际请求和响应不被改写。认证头、凭据服务内容、客户端 metadata、供应商错误原文不进入正文；正文始终不写日志。SQLite 正文没有静态加密，数据库备份会包含已经录下的内容；关闭开关不清除历史或备份。

详情按需读取正文，列表不带大文本；支持完整角色列表、回答、录制范围标记与刷新，长文本滚动查看而不截断。旧记录明确显示未录正文。文本按 React 纯文本显示，不能执行返回的 HTML。设置保存结果不确定时显示未知并提供重新读取，不以本机乐观状态猜测真实录制状态。

“全部”指 LocalMux 在本轮接口中实际收到的可见对话文本；不扫描第三方历史，不抓取上游未返回的内部推理、工具过程或私有日志。Responses 各轮仍按原 conversation_id 关联；新调用携带的历史可被本轮录下，但不反向改写旧记录。

## 有界运行诊断（2026-10-10 / M3）

`0013` 在既有 call_records 添加可空 request_id 和 diagnostics JSON（固定类型、最多 8192 字节）；旧行保持 NULL，不回填。入站关联 ID 由 Gateway 生成，不能从客户端头注入。阶段时间使用单调时钟，accepted_at 用 Unix 秒：auth_route_ms 是入站到 lane 前的本机预检；queue_wait_ms 只测锁等待，interval_wait_ms 单测启动间隔；adapter_startup_ms 是流方法到返回可接收句柄的本机时间，adapter_ms 是调用/接收直到 EOF/异常的本机耗时，含 startup，不代表纯云端推理；first_output_ms 只在 SSE 的非空文字 delta 可观测时填，非流首字与 model_execution_ms 未观测则 NULL。total_elapsed_ms/旧 elapsed_ms 均从 admission 到终态快照、在数据库终态提交之前测量。response_finalize_ms 的终态写入耗时仅在内存完成后可得，持久快照为 NULL，不补 0。

request_bytes 是 Gateway 收到的 JSON Body 字节；非流 response_bytes 是已编码 Body，SSE 是到首个终态帧为止的本机 Body 字节前缀（scope=through_first_terminal_frame），不当作完整 socket 传输/消费端确认。output_text_bytes 是已接收文字字节。HTTP 状态、固定 reason/outcome、用量来源/口径、规范化、是否录正文与取消请求分别记录；取消远端可证性始终 UNKNOWN。规范化可以与完成/失败共存，DIRECT_COMPLETED 只指正常接收终态，不推断模型 stop。store:false 完全绕过元数据和正文，包括新诊断，无新 telemetry 表；prompt、RPC、原始错误、凭据和参数均不在诊断中。

本机 IPC 连接状态只根据已注册 Adapter 的文件/健康、lane 占用、执行计数及冷却得出 ACTIVE/IDLE/BUSY/COOLDOWN/UNAVAILABLE，含来源/观测时间；不自动运行模型或扩大授权。HTTP 能力目录只对 Key 已可见型号附带本机状态。客户端应把超过 30 秒的 GUI 状态视为过期，空闲不是账户/型号推理通过。

回滚需要先停止且确认任务为 0，保留升级后 DB/正文备份，再恢复 M3 前 DB 与匹配旧程序；旧 SQLx 程序不能直接打开含 0013 的数据库。不要在用户数据库删列/篡改迁移账本，升级后新增记录先保留。

2026-10-14：能力目录 `chat.details.session_resume.api_support` 分别报告 Chat（UNSUPPORTED）和 Responses 的当前 Adapter 契约；不替代本节 owner、最新成功响应、模型/Adapter 黏性和原生 ID 门禁。prompt_cache_control 在两入口均 UNSUPPORTED；报告 cached tokens 与续接/正文录制是独立事实，无全局隐藏会话，无新迁移或历史回填。
