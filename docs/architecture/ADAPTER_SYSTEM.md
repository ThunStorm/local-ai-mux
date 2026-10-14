# Adapter 契约

`localmux-adapter-sdk::Adapter` 提供 `id/kind/health/list_models/get_quotas/invoke/invoke_stream`，输入与输出分别是 `NormalizedRequest`、`NormalizedResponse` 与增量 `NormalizedStream`。旧 `get_quota` 由默认 `get_quotas` 包装，Codex 分别报告多个窗口。真实 Agent/Provider 实现必须返回可验证模型能力与统一 `AdapterError`，不能在 Router/GUI 散布 Provider 分支，也不能把供应商异常或凭据原文直接发给 API 消费者。

Phase 1 的 `MockEcho` 只用于离线验收，生产桌面 Gateway 不预置 Mock。Codex 使用官方 App Server；Qoder 与 CodeBuddy 使用各自官方 headless CLI，并复用 `adapter-sdk::cli` 的 JSONL 进程管理。桌面控制层用静态 `ADAPTERS` 注册表统一 CLI 文件名和构造函数；每个已显式添加的 Adapter 才注册进 Gateway 动态目录。费用状态没有上游证据时一律 `UNKNOWN`。本机已安装 Qoder 与独立 CodeBuddy Code CLI；WorkBuddy 内部脚本不等同独立 CLI，离线协议测试仍不代表真实连接验收。

MiMo 开放平台走用户显式提供的 API Key 和固定 HTTPS 官方端点，使用 `HttpProvider`；MiMo Desktop 的安装登记与此连接无关。macOS 独立 CodeBuddy Code CLI 可从 NVM 的 bin 目录发现；WorkBuddy 包内脚本缺少完整 config 命令产物，保持独立桌面登记。没有可调用 CLI 的 Qoder 桌面包不会被直接注册。
MiMo Code CLI 的公开 `mimo serve` / `mimo llm-server issue` 提供独立的回环 OpenAI 文本能力 API；用户点击连接后，桌面控制层启动临时本机服务，先取真实目录，再将短时令牌限制到 `mimo/`、`xiaomi/` 型号。连接仅在本次运行的内存中有效，断开时撤销令牌；不读取 MiMo Desktop 登录态，也不与开放平台 Key 混用。HTTP Provider 在目录验证后可作为降级健康候选进入 Gateway。
Qoder 的模型目录命令返回未登录时明确报认证缺失；本机 1.1.64 登录后即使指定 `--output-format json` 仍返回 `MODEL` 表头的逐行文本，Adapter 按真实格式保留目录报告的模型名。全球版 `qoder` 和中国版 `qodercn` 使用同一公开 CLI 协议，但注册为独立 Adapter ID、路径与模型前缀，不共享登录态。WorkBuddy 无可验证模型 CLI，CodeBuddy CN 的 `code` 是编辑器启动器；独立 CodeBuddy Code CLI 在未设置模型覆盖时可解析其公开帮助目录，格式无法核实则拒绝。桌面只读取官方 `qoder status --output json`、`qodercn status --output json` 的 `logged_in` 布尔值及 `codex login status` 的登录结论；不展示或记录账号标识。只有确认需要登录才出现登录入口，启动后限频轮询自动确认，目录连接和真实模型短测试均需单独点击。

`NormalizedRequest` 同时保留 Responses 单文本 `prompt` 和 Chat 的有序 `messages`。Adapter 必须使用消息数组或在发起上游调用前通过 `single_user_prompt()` 拒绝未支持的角色/历史；不能忽略 `messages` 后返回成功。HTTP Provider 发送完整消息数组，现有 Agent 桥的增量前置指令契约见下方能力协商实施；历史不自动 flatten。

2026-10-04：`NormalizedRequest.parameters` 保留可选 temperature/top_p/max_tokens；`ModelDescriptor.chat` 描述角色/历史能力、参数 support/min/max 与来源。默认 UNKNOWN，单轮 Agent 桥的历史/system/采样明确 UNSUPPORTED。Gateway 和 HTTP Adapter 都验证能力，未知字段不透传、已提交但不可支持的字段明确报错；未启用字段省略。界面使用能力描述，不按型号名称猜功能。

HTTP 目录可带公开模型元数据；MiMo Code 在用户显式连接的 loopback 服务上读取安装官方 SDK 已定义的 `GET /provider`，按 provider/model 精确匹配 temperature 与 limit.output，不跨 Provider 借用同名模型能力，元数据读取不携带推理令牌。无元数据、失败、无合法上限时保持 UNKNOWN。安装 CLI 的公开请求 schema/透传实现与本地 fixture 用于合同核对，不等于真实模型验收；厂商网页本轮返回 404，因此另保存了本机 SDK 依据和这个限制。

2026-10-06：Codex thread start/resume 的 `sandbox` 使用官方枚举 `read-only`，turn 的 `sandboxPolicy.type` 使用 `readOnly`。CodeBuddy 在隔离目录优先查询显式模型覆盖，未设置时解析官方 CLI `--help` 的明确模型列表；仅能证明 CLI 能力目录，不推断账户授权、桌面订阅共享或免费。真实连接保存仍由用户点击发起。

Codex 子进程在 macOS 缺少显式代理环境时遵从现有系统固定 HTTP(S) 代理，loopback 绕过，凭据不读取/落库；不自动更改代理设置。官方错误通知及时结束当前调用并保留安全分类。Gateway 与桌面流式等待共享 120 秒空闲界限、心跳不算回答；熔断连接与不存在的模型分别报告。

2026-10-06 能力协商实施：NormalizedResponse 增加 ResponseMetadata，新增 EventStream（delta/usage/completed）与 SessionEventStream，旧 NormalizedStream 文字接口仅为兼容包装。Codex/Qoder/CN/CodeBuddy/HTTP 的新链路统一到 typed 流；非流式从同一事件收集元数据。未知 usage/finish/actual_model 不补 0/stop/猜测型号，供应商累积计数不加到请求快照。ChatParameters 支持 max_completion_tokens、reasoning_effort、标准 response_format，但只有已映射且满足精确约束的值可用。CLI 映射前置 system，Codex 独立 base/developer 字段；未支持角色/格式/上限明确拒绝。能力来源与实施边界见[实施验收](CAPABILITY_NEGOTIATION_IMPLEMENTATION_2026-10-06.md)，旧 single_turn 模板不得作为底层模型不支持的证明。Tools/config 约束不改变 AdapterKind 或 Key 的 Agent 授权。


2026-10-08：CLI/Codex 等待输出事件的空闲上限为 180 秒；Gateway 另有 600 秒总期与 Agent 连接串行，取代早期 Gateway 120 秒整体等待。AdapterError::Timeout 与 ProcessFailed 分开，映射 LMX_PROCESS_TIMEOUT；错误 result 的额度/鉴权分类只在内存进行，不透传 stdout/stderr。取消 drop 终止本机句柄，但远端取消/消费仍待真实验收。连接锁覆盖 Chat、Responses、模型验证；详情见[Streaming](../api/STREAMING.md#2026-10-08-cli-调用期限与并发修订)。

2026-10-10 M2：复用五分钟目录缓存和 CapabilityDetail；公开版本/fixture 观察增加到期时间与未知理由。缓存刷新或显式失效后重新读取当前公开目录；过期支持证据转 UNKNOWN，不根据型号名称、答案自述或同族网页推断参数支持。读取公开目录不触发生成，不借 catalog 观察标记 live_verified。v2 新字段可缺省，无迁移；回滚移除兼容扩展即可。

2026-10-10 M4：ResponseMetadata 增加可选 actual_model_source/finish_source，Gateway 对来源、型号长度/字符、规范化名称及 usage source/scope 做白名单检查。HTTP 标准元数据来源为 provider_reported；Codex thread model 与 rerouted 分别登记 app_server_thread/app_server_rerouted，未知 finish 仍 null。第三方 Adapter 未标来源的 stop/model/usage 不得成为对外已知事实；协议 Completed 仍可以证明接收结束，无需伪造 stop。无新增数据库迁移，旧标准客户端可继续忽略 localmux。

2026-10-10 M5：shared cli::read_json_line 在读取时限1MiB，目录 stdout/stderr 同样各限1MiB，Codex 等待响应的 pending 通知另限4MiB；EOF/异常 JSON/非零退出不能冒充 Completed。Codex 版本读取与 CodeBuddy 目录复用有界且隔离 cwd 的公开命令执行。Unix 使用为本请求创建的新进程组；Drop 只在 Child.id 仍有效且 getpgid=自身PID时清理组，已 wait/reap 的句柄不使用旧PID，保留 Tokio 直接子进程清理。离线 fixture 验证父子停止和无关进程保留；主动逃离进程组的后代、Windows 进程树与云端停止仍 NOT_VERIFIED/UNKNOWN。不杀名称匹配的进程，不盲重试。回滚 M5 不涉及 SQLite/授权，只回退 SDK/Adapter 代码；部署仍须任务0。

2026-10-14 Codex：移除 system/developer/effort 的精确版本白名单，改从当前安装 CLI 自动导出非 experimental 公开 Schema，逐字段核对 start/resume/turn 映射，推理档位取协议与型号目录交集。导出失败不隐藏用户模型，能力保持 UNKNOWN 并记录安全原因，默认单 user 省略未核对指令；用户前置规则不可丢弃。10秒 owned 进程、三个普通文件各1MiB、本地引用深度8、临时清理与原五分钟 Gateway 目录刷新共同限定开销；真实效果仍未验证，不跨应用自动改快照。详见 [Codex Adapter](../adapters/CODEX.md#supported-versions)。

2026-10-14：缓存细分经统一 TokenUsage 校验，只接受可信 request/last_turn 且 cached≤input；未知不补零、不累加。typed 收集与 Gateway wire 复用此校验，累计终态不覆盖最后可信快照。能力 details 增加可选 api_support，分别声明 Chat/Responses 原生续接和缓存控制；Codex 另核对公开 resume.threadId，CLI 桥核对 --resume，HTTP Chat 桥不实现 native resume。缓存控制由 strict schema 限制为 UNSUPPORTED，底层保持 UNKNOWN；声明不改变请求/路由权限，也不标记真实生成通过。见[本轮计划](../LOCALAIMUX_PREFIX_CACHE_AND_SESSION_CAPABILITY_ASSESSMENT.md)。
