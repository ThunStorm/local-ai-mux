# 通用能力协商实施与验收

范围依据：[设计说明书](CAPABILITY_NEGOTIATION_DESIGN.md)。本次实施 LocalAiMux 的 M1、M2 最小文本契约、M3 响应规范与 M5 离线互操作。至简 Z1–Z4 不在本仓库，未修改；M4 history/视觉/工具的新功能、真实推理、所有型号参数效果、远端取消/计费、Windows 实机均不属于本轮通过结论。

## 实现边界

| 项目 | 已实施 | 保留限制 |
| --- | --- | --- |
| M1 目录 | 沿用 ModelDescriptor/ChatCapabilities；`/api/chat/models` schema_version=2，保持 Key/Agent/连接过滤和旧有效字段；新增层级来源、版本、时间、约束、native 模态与逐型号 effort | 身份仍为 adapter compatibility scope，未重复实现 connection instance 迁移；所有新证据 live_verified=false，不继承历史推理验证 |
| M2 消息 | 单条 user 或连续前置 system + user；同角色指令按顺序以显式分隔合并；Codex 另外保留独立 developer 层 | CLI 单 system 层拒绝 developer；system 出现在正文后、历史导入、developer 后再放 system 均拒绝，不 flatten 历史 |
| M2 参数 | 接受标准 max_completion_tokens、reasoning_effort、response_format；两个输出上限字段冲突必拒绝；仅 Codex 当前核对的 0.160.0 + model/list 精确枚举可映射 effort，low 不宣称 off；text 格式可用 | 输出上限没有已验证口径和型号范围时拒绝；temperature/top_p 保持精确能力门禁；没有私有 thinking/context 参数后门 |
| 输出格式候选 | Codex outputSchema、CodeBuddy --json-schema 和 Qoder 输出控制的公开接口证据分别登记 | Schema 子集/最终失败/约束保证和 Qoder 各型号限额仍缺证据，json_object/json_schema 继续明确 not_implemented；不把 stream-json 当模型 JSON |
| M3 响应 | 新增 typed delta/usage/completed，保留文字流兼容包装；Codex `last` 快照、HTTP 标准 usage、实际上游已报告型号、未知结束原因与标准 length/content_filter | 不读取 Qoder/CodeBuddy 累积 model_usage 来凑 per-request 用量；CLI 元数据无核对来源时 null；正常协议完成不等于 stop；未报告实际型号则 null |
| 执行/错误 | CLI 独立 cwd、tools/MCP 禁用、setting-sources 空值；Codex 请求级公开配置禁用 shell/exec/apps/plugins/多 Agent/浏览器/Computer Use，逐个禁用公开配置层已报告的 MCP；拒绝工具 item；配置错误含 param/reason_code/limiting_layer/retryable=false/request_id | 保留 Agent 身份和既有显式 Key 权限，不改成 Provider 绕权。Codex 官方配置覆盖与工具禁用的运行效果尚未真实推理验证；本轮不声称远端无工具/取消停止计费已验收 |
| 桌面终态 | length、过滤/拒绝、取消不进入成功历史，保留部分文字和安全中文提示 | 试对话参数面板沿用已有控件；不捆绑新的 effort/Schema/多媒体/会话 UI |

CLI 的 user 正文仍走 stdin；公开 system-prompt 旗标携带前置指令，因此该层指令可见于本机进程参数，但不进入 SQLite、应用日志或错误正文。调用方不应在提示词中放凭据。Codex 指令通过 stdio JSON-RPC，供应商配置仅通过公开 config/read 读取配置层，并只提取 MCP 名称构造禁用项，不复制凭据/命令值、不写供应商全局配置。公开配置项依据[官方配置参考](https://learn.chatgpt.com/docs/config-file/config-reference)，协议字段以安装版 generate-json-schema 为准。

## 验证证据

本机显式路径、只读目录测试：`cargo test -p localmux-gateway --test installed_capability_catalog -- --ignored --nocapture`，四个 `LOCALMUX_CATALOG_*` 变量指定数据库已有的独立 CLI。Codex 7、Qoder 17、Qoder CN 14、CodeBuddy 17，55 个唯一 ID 与原审计完全一致；七个 Codex 型号的输入模态、默认 effort 和合法枚举逐项一致。只有目录/help/version；没有 thread/start、turn/start、推理、保存连接或账号扩权。脱敏证据：[目录复核](capability-negotiation-validation-20261006.json)。该测试默认 ignored，CI 不访问真实账号。

离线回归包括前置角色组合/优先级、参数冲突/未知/范围、不支持历史、真实模型目录的 Key 过滤/撤销、Codex 双指令字段与 effort、重复 last 快照而不累计 total、HTTP 标准元数据、Chat/Responses 非流式与 SSE 截断一致性、未知 usage/finish 的 null、协议中断/超时/冷却和桌面不把部分回答入历史。Qoder/CN 和 CodeBuddy 均通过公开形状 fixture，不能替代其真实推理效果。

官方 Python OpenAI SDK 3.24.0、TypeScript SDK 7.23.0（pnpm 锁文件版本）针对合成 Mock Gateway 验证同一通用 system/user + text 请求、unknown usage/finish、include_usage SSE、双上限冲突诊断，并回归 models/Chat/Responses/两种流。合成 Key 与数据库只在隔离测试进程中，不使用安装版凭据或真实 Provider；没有客户端特判。

`bash scripts/verify.sh` 最终通过：Rust 78 passed / 0 failed / 2 个显式路径真实目录测试默认 ignored，前端 7 passed，格式/类型/构建通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过。两个官方 SDK smoke 在最终构建上复跑通过。完整本机安装记录与最终 hash 在[交接](../HANDOFF.md)。初轮 HTTP 回归因错误枚举从旧 UnsupportedParameter 升级为带字段的 InvalidParameter 而需更新断言；首个工作区构建遇到旧工作树的 Tauri permissions 缓存，仅 `cargo clean -p tauri` 后重编译，未改应用数据。release 打包也发现同类路径，另外清理 release 的 Tauri/直接插件/桌面生成缓存后重建；认证/限流共用 RPC 错误码的分类回归由既有 fixture 拦截，修正为具体认证/限流/额度类别优先于通用参数错误，再全量复跑。

## 尚未验收与后续准入

真实生成需用户在已授权连接上显式发起，串行代表样本后才能更新对应能力的 live_verified，不批量消费 55 型号。Qoder/CN 官方 SDK 尚未版本配对，精确窗口/输出/思考值继续 UNKNOWN；CodeBuddy 亦不借同族原生服务 API 推断订阅出口。Schema 开放需先完成 schema 子集、答案字段、内部修复、最终失败及禁用工具模式的独立验收；硬 Token 上限须取得精确型号范围和计数口径。费用/额度、实际路由与取消保证的未知状态不得用答案自述、0 或 stop 填补。
