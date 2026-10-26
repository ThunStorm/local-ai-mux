# 流式响应

## v0.2.0 当前计时契约（2026-10-26）

Gateway总期限从入站起1200秒，覆盖鉴权/路由、排队、启动、执行、终态和SSE背压；15秒传输心跳不延长总期。CLI/Agent typed事件空闲可在桌面API页配置30–1200秒，默认180、现有已保存值保留；每次入站取快照，新保存仅影响新请求，启动恢复。CLI等待完整输出事件；Gateway等待typed事件，内部非文字通知可能不成为typed事件。

| 层 | 当前限值 |
|---|---|
| gateway_overall | 1200秒总期 |
| cli_idle / gateway_stream_idle（Agent） | 当前保存的30–1200秒 |
| gateway_stream_idle（HTTP Provider） | 180秒 |
| gateway_startup | 30秒，受剩余总预算约束 |
| cli_rpc | Codex控制响应每次读取15秒 |
| cli_command / cli_exit | 命令各自固定限值 / EOF后退出5秒 |
| Agent队列/间隔 | 30秒队列（429）/5秒启动间隔/每连接串行 |
| HTTP Adapter / 桌面试对话客户端 | 各自120秒总限时，独立于CLI设置 |

不能把配置为1200秒解释为任意客户端都保证等待满20分钟；先触发的真实层决定结果。LMX_PROCESS_TIMEOUT的固定timeout_layer/timeout_limit_ms与diagnostics保持同源；未提供层的兼容错误保留unknown。只保证本机接收/owned进程清理，远端执行/计费未知。

以下按日期保留演进记录；600秒总期与旧范围是历史参数，被本节覆盖。

Chat 使用 `text/event-stream`，发送角色、文本增量、已报告 finish_reason（未知为 null）和 `data: [DONE]`。Mock 返回单个文本块；Codex Adapter 的 `item/agentMessage/delta` 会逐块映射为 SSE。首块与后续块有超时限制，客户端断开时流被丢弃，所拥有的子进程会终止。官方 Python/TypeScript SDK 已对 Mock 路径消费成功；Codex 真实订阅流仍待显式连接后验收。

2026-10-04 桌面试对话通过 Rust HTTP client 请求固定 loopback Gateway，使用 Tauri Channel 传 started/delta/completed/failed/stopped；每条事件含窗口所属请求 ID、递增序号、固定模型和本机耗时。前端忽略旧请求/重复/不同模型事件。SSE 解码处理 UTF-8 分片、CRLF 和多行 data；收到 `[DONE]` 且有有效文字才完成，异常 EOF 或上游 error 不把部分回答判完成。

桌面总请求上限 120 秒，SSE 块等待与 Gateway 首个文字增量/后续增量的空闲上限统一为 120 秒，单帧缓冲上限 1 MiB、答案接收上限 4 MiB；超过上限显式失败，不静默截断。取消按窗口与请求归属匹配，先丢弃 HTTP/SSE future、释放占用，再发终态；窗口退出在有任务时被拒绝。离线测试证明取消接收会释放本机服务器流。不能由本机连接关闭推断远端推理立即停止或停止计费；GUI 明确提示上游可能继续执行。真实 Provider 子进程/取消效果仍需实际验收。

2026-10-06：Chat/Responses SSE 在等待上游时每 15 秒发送注释心跳；心跳只表示连接仍在，不计作文字/完成。桌面总期限仍为 120 秒，启动 Adapter 流的门限仍为 30 秒；Gateway 流空闲超时报告 `LMX_PROCESS_TIMEOUT`，桌面映射为 `LMX_TIMEOUT`。Codex 的官方 `error` 通知/failed turn 会保留安全错误类别并结束本次子进程，不忽略连接错误等待隐式重试。上游失败的鉴权/额度/限流类别不统一吞成普通失败。

2026-10-06：生产 Adapter 改用 typed delta/usage/completed；EOF 缺少 completed 时失败，旧文字流 Adapter 兼容包装的 completed 只能证明接收完成，finish 仍 unknown。usage 快照替换而不相加；Chat 仅在标准 include_usage 选项启用时发送最终 usage 块，unknown=null。Responses 使用相同元数据，length/content_filter/cancelled 返回 incomplete，不保存为可续接的成功响应；不能发送 response.completed 掩盖截断。桌面识别 length/过滤/取消终态，保留部分文字而不纳入成功历史。协议错误、超时、截断、结束原因与 `[DONE]` 的传输终止分开。


## 2026-10-08 CLI 调用期限与并发修订

Gateway Chat/Responses 非流式、原生续接和 SSE 使用 600 秒总执行期限；Adapter 流启动仍 30 秒，首个/后续事件空闲上限改为 180 秒，连续增量不重置总期限。等待 CLI 连接锁最多 30 秒（另有最多 5 秒启动节流），执行期限从取到锁并完成节流后开始；目录/授权检查不纳入该期限。SDK CLI/Codex stdout 无事件等待也为 180 秒；没有输出的 CLI 超时报告 LMX_PROCESS_TIMEOUT，不冒充普通进程故障。SSE 心跳仍只代表传输连接，不证明模型进度。桌面 playground 保留原 120 秒客户端总期，因此本次 600 秒用于直接 Gateway 客户端，并不自动放宽桌面交互。

同一 Agent Adapter 连接（当前一 Adapter 一账户）跨模型、客户端 Key、Chat/Responses 和显式模型验证共用并发 1、启动间隔 5 秒；锁等待超时返回 429 LMX_RATE_LIMITED 与 Retry-After=5，不启动上游，不触发供应商冷却。失败、超时、客户端断流和取消释放占用；等待中的请求计入 active_tasks，退出/部署门禁继续生效。独立 CLI/官方应用绕过 Mux，不能视作全账号锁；未来多账户实例须按真实连接 ID 分开。

离线验收入口：crates/gateway/tests/stream_failures.rs 使用虚拟时钟证明 130 秒非流式可完成、601 秒总期失败、持续增量仍在总期收口、跨协议/模型/Key 队列和断流释放；不产生真实推理。远端取消/计费和真实厂商限额继续 UNKNOWN。

2026-10-08 调用审计与流生命周期：记录 guard 跟随整个请求及 SSE Body，未开始消费/消费中被丢弃均可记 cancelled；typed usage/completed 直接更新元数据，协议无 completed 的 EOF 记 interrupted。上游 length/过滤/取消均保留其终态记录，但不创建成功 Responses 续接映射；本次已知完整结果在客户端关闭时保留原终态。补写为异步，进程异常退出前未落库的 in_progress 在下次启动恢复。`store:false` 完全绕过记录层，原协议帧与取消/队列行为不变。记录不保证远端取消或停止计费，也不修改当前工作树的时间预算。

调用记录的已知终态在完成帧前持久化，SDK 收到 completed/[DONE] 后停止读取仍可立即查询终态。SQLite 终态写入失败会中断传输，不继续发送成功完成帧；客户端在终态之前主动断开时，取消补写仍为异步。

2026-10-09：正文录制开启时，已接受 delta 在转发前写入跨分片脱敏快照，停止/超时/异常 EOF 保留已收到的文字。设置关闭事务及输出代次条件阻止关闭后的新正文写入；客户端接收仍可继续，录制开关不取消推理或计费。存储失败返回安全内部错误或失败流，不发送虚假成功。部分正文不转化为完整上下文，原 native resume 与终态规则保留。

## 2026-10-10 入站总预算（M1）

600 秒现在从 Gateway HTTP middleware 接受请求开始，覆盖鉴权、schema/目录/路由、30 秒 lane 等待、5 秒启动间隔、30 秒 Adapter 流启动、实际接收及终态持久化/响应编码。子期限取剩余预算与原上限的较小值；不改变默认 600/180/30/5。返回 HTTP 头 `X-Request-ID` 是 Gateway 新建的安全 ID，不信任调用方提供的 ID。超期返回既有 504 LMX_PROCESS_TIMEOUT；总期先于 queue 到期时不能误报本机忙。心跳不延长总期，慢客户端暂停消费也不能在重新消费时收到逾期成功帧。断线丢弃 owned stream/lane；仅保证本机清理，远端取消/计费 UNKNOWN。

显式模型短测试同样从内部入口开始计时；桌面 120 秒客户端期限不变。ActiveRun 从 HTTP admission 覆盖预检和等待，SSE Body 丢弃后释放；非流/错误在生成响应后结束。错误 Body 可在总期到达后传递安全超时诊断，不继续启动上游。回滚 M1 可恢复旧的执行后起算语义，无数据库迁移，Key/Scope/连接不变。

2026-10-10 M1 门禁补强：流生产者独立拥有单调 deadline、原 SSE Body/CallGuard/Adapter/lane，以最多单帧缓冲转发；总期包括等待客户端背压，Body 从未被消费也会到期释放资源。外部 Body 丢弃会 abort 生产任务，清理在 Tokio 下一调度发生；迟到缓存帧不能冒充完成。标准 15 秒注释心跳与180秒上游空闲保持不变。取消指 Gateway 实际观察到的 HTTP 对端关闭；客户端通过代理且代理保留连接时，不能推断原客户端已经断开，仍受总期限保护。隔离 SDK 测试固定 NO_PROXY=127.0.0.1,localhost，不更改用户代理配置。

## CLI 空闲限时与超时归因（2026-10-21）

API 页「调用限时」可保存 `cli_idle_timeout_seconds`，整数 30–600 秒，默认 180；写入既有 settings 表，无迁移。启动恢复设置，读取/保存失败明确报错；在 HTTP 入站或模型短测试开始时取快照，仅新请求采用新值，执行中的任务不变。Codex 生成事件、Qoder/CN/CodeBuddy JSONL 读取使用该快照；等待完整事件而非单个字节，任一完整 CLI 事件重新计时。Agent 的 Gateway typed 上游事件等待也使用该值，Provider HTTP 路径仍为 180 秒；CLI 非文字通知不等于 Gateway typed 事件，哪个层先到期就记录哪个层。SSE 注释心跳不延长这些等待。

| 超时层 timeout_layer | 限值 / 观测范围 |
| --- | --- |
| cli_idle | 本次配置；下一条完整 CLI 输出事件 |
| cli_rpc | Codex 控制响应每次读取 15 秒 |
| cli_command | 公开目录/版本命令自己的固定限值，按实际触发计时器记录 |
| cli_exit | JSONL EOF 后等待进程退出 5 秒 |
| gateway_startup | 流式 Adapter 启动 30 秒，受剩余总预算约束 |
| gateway_stream_idle | 下一条 typed 上游事件；Agent 同 CLI 配置，HTTP Provider 180 秒 |
| gateway_overall | 入站起 600 秒，包含预检、排队、间隔、上游与流式背压 |
| unknown | 未提供具体层的兼容 Adapter 超时；不推测为总期限 |

非流式错误、Chat SSE、Responses failed 保留 `LMX_PROCESS_TIMEOUT`，兼容新增 `timeout_layer`、`timeout_limit_ms`、`reason_code` 与 `limiting_layer`；HTTP 200 之后仍通过错误帧报告。新 call_records.diagnostics 同源保存触发层、限值与本次 CLI 设置；旧行/旧 JSON 缺键保持未知，不回填。`store:false` 仍不保存。没有变更自动重试、路由或权限。

其他限制独立存在：Agent 队列 30 秒为本机忙 429；HTTP Adapter 请求总限时 120 秒；桌面试对话客户端总限时 120 秒，外部客户端可更早断开。CLI 配置不延长这些限制，Gateway 仅观察到断开时记取消，不能据此反推客户端计时器或远端执行/计费状态。各计时器以单调时间运行，同刻到期时记录本机实际观察的触发层。
