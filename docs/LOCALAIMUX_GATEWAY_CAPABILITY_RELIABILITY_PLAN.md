# LocalAiMux 独立实施计划：通用能力契约、Deadline、Telemetry 与 Adapter 可靠性

> **交付类型**：可直接交给 LocalAiMux 项目 Codex/Agent 实施的工程计划  
> **目标仓库**：`ThunStorm/local-ai-mux`  
> **目标分支**：`codex/dev`  
> **基线复核日期**：2026-10-10（基于目标分支源码及其 2026-10-09 前后的实现/交接文档；非实时生产验收）  
> **唯一修改范围**：LocalAiMux 本仓库。**不得修改至简 `ThunStorm/do-not-litter`。**  
> **独立交付原则**：单独运行、单独测试、单独发布。所有新增诊断字段都是可选的；不要求任何消费端同步升级。

---

## 0. Agent 执行授权和边界

你是 LocalAiMux 实施 Agent。按照 **M0 → M1 → M2 → M3 → M4 → M5 → M6 → M7** 分阶段完成，每一阶段维护小范围 diff、针对性测试、全量验证与回滚说明。**只处理本仓库的通用模型服务能力。**

### 开始前必须执行

1. 确认当前 HEAD、分支、未提交文件，先阅读仓库的 Agent/开发规范、`docs/HANDOFF.md`、`docs/api/OPENAI_COMPAT.md`、`docs/api/STREAMING.md`、`docs/architecture/CAPABILITY_NEGOTIATION_DESIGN.md` 与 `docs/architecture/CAPABILITY_NEGOTIATION_IMPLEMENTATION_2026-10-06.md`。不为了解背景全仓扫描。
2. 先复核本文件中的“已实现/待增强”事实。若相关能力已经实现，只补齐实测缺口或明确缺失的边界，不再并行建立第二套 capability catalog、历史记录系统或 quota registry。
3. 保持目前已有 Tauri 桌面 GUI、Rust Gateway、Adapter SDK、SQLite、原生 CLI/App Server 的工程路线；不重写为 Web 服务器管理控制台或命令行产品。
4. 默认只进行源码、mock/fixture、单元与集成离线测试、文档。不得未经用户明确同意执行真实 Provider 生成、消费订阅额度、重新授权/升级权限、批量测试 55 个型号、编辑厂商私有配置、获取隐藏 token、重建 API Key、生产重装或自动重启。
5. 保护现有 Key/Scope/连接、路由策略、速率限制与订阅账号授权边界；**不得通过 UI 自动化、内部未公开接口或凭据解密来绕开官方限制**。
6. 不向调用方暴露原始凭据、完整 Prompt、Agent 内部 RPC 内容、未授权模型信息；记录正文仍按现有显式 opt-in、默认关闭策略运行。
7. 严格区分离线证据、真实本机实测和厂商未证实行为。`UNKNOWN`、`NOT_TESTED` 不得写作 `PASS`。

### 要求的 Agent 产出

每个 M 阶段结束，提供：变更摘要、被复用的旧实现、回归/构建结果、兼容契约、真实性证据、风险、回滚步骤、更新的源码/文档位置。未经用户要求不提前部署、不推送或发布 Release；按仓库当前 Git 流程和用户当次授权处理。

---

## 1. 产品定位与现状核对

### 1.1 定位

LocalAiMux 是将**用户已经合法授权的本地桌面 AI/CLI 能力**通过通用本地 OpenAI 兼容服务提供给其他客户端的桌面应用。它负责模型目录与能力、Adapter 身份与连接健康、通用 API、队列/执行、错误、用量状态和安全，不负责调用者的业务 Prompt、旅行 POI、视频片段或任务工作流。

消费端可以是至简，也可以是其他标准 OpenAI SDK 客户端。**不能为至简定制一条私有调用链或发明业务专用字段。**

### 1.2 当前已经有的能力（不得重建）

| 已有功能 | 入口/事实 | 本轮只补 |
| --- | --- | --- |
| `/v1/models`、`/v1/chat/completions`、`/v1/responses` | `crates/gateway/src/lib.rs` 与 `docs/api/OPENAI_COMPAT.md` | 边界与一致性回归 |
| `/api/chat/models` `schema_version=2`，带接口来源/限制 | 通用 capability negotiation 已落地一批 | 来源精度、陈旧状态和真实能力证据 |
| `system`/`user` 最小契约、参数支持/拒绝 | ChatRequest `deny_unknown_fields` 与 Adapter 适配 | 精确型号/版本的行为测试 |
| Chat 与 SSE 规范化，usage/finish nullable | `crates/api-types/src/lib.rs`、Gateway | 真实上游来源/耗时/恢复语义 |
| `localmux` 扩展响应元数据 | Chat 响应已存在该节点 | 字段稳定、来源可信、字节可控 |
| CLI Agent 队列/间隔 | `AGENT_QUEUE_TIMEOUT=30s`、`AGENT_REQUEST_INTERVAL=5s`、单 lane | 时间预算一致性、可解释队列耗时 |
| 总请求/流空闲保护 | Gateway 文档当前为 600s/180s | 各层 Deadline 与取消/终态证据 |
| 本机调用记录与 GUI | 默认元数据保存、正文显式 opt-in；`store:false` 隐私语义 | 观测字段、可定位故障及脱敏 |
| Codex/Qoder/CodeBuddy Adapter | 本仓库多 Adapter 与发现能力 | 逐个版本、实际模型的可靠性验证 |
| Mock 与 Python/TS SDK 测试 | `scripts/verify.sh`、已有 mock/fixture | 队列、真实元数据、能力回归矩阵 |

注意：`docs/api/OPENAI_COMPAT.md` 明确记载：当前 `response_format=json_object/json_schema` 并未因请求字段可接受就证明各 Adapter 具有原生结构化保证；`usage=null` 和 `finish_reason=null` 是有效未知状态。**本计划禁止把客户端自行解析 JSON 的成功冒充为 Mux 原生 Schema 支持。**

### 1.3 与至简的清晰分工

| 问题 | LocalAiMux 的职责 | 不属于 LocalAiMux |
| --- | --- | --- |
| deadline | Gateway 入站、队列、Adapter 运行、SSE、断线取消的时间与终态 | 视频 Job/Stage 的业务 deadline |
| capability | 发布通用模型/接口 capability catalog 与精确 evidence/provenance | “某模型是否适合识别旅游 POI/校对 ASR”的业务能力评级 |
| telemetry | HTTP/Adapter 处理、排队、首字、实际型号、用量来源、错误 | 逐 Video Segment、Claim/POI 的业务统计 |
| repaired/direct success | 记录**通用协议正规化**和原生/推断的来源，成功状态真实可信 | 旅游 JSON 修复、业务 Evidence 解析分类 |
| Stage 状态机 | 单次请求的 Gateway/Adapter 生命周期 | 客户端的跨 Stage Job 状态机 |
| 回归基准 | SDK/协议/排队/Adapter 的 mock Golden | 视频 POI Precision/Recall |
| 用户体验 | App 扫描/连接/状态/复制通用 API 信息 | 至简的视频笔记/地图/任务流程 |

**独立验收条件**：使用 Mock Adapter 与官方 Python/TS OpenAI SDK，通过本计划全部离线验收；完全不需启动至简。至简的最终跨项目 E2E 由至简独立计划的 `E2E` 章节发起，Mux Agent 只需提供标准 HTTP 服务和已完成的脱敏测试证据。

---

## 2. M0 — 接口、运行时与观测的基线审计（P0）

先建立无需真实订阅调用的 baseline，避免重复修改现有机制。

### 核对清单

- 核对当前 API 形状：
  - `GET /v1/models`：受 Key/scope/连接权限过滤的标准 OpenAI 模型列表；
  - `GET /api/chat/models`：`schema_version=2`、id、provider、adapter/identity、`chat` 及来源/生效支持状态；
  - `GET /api/health`：现有健康与 `execution_limits`；
  - `POST /v1/chat/completions`：标准 roles、text、`store`、stream/非 stream、optional params；
  - `POST /v1/responses`：目前只允许的文本/原生会话续接语义；
  - 保留 `localmux` 扩展，仅在成功确实可知时填写真实字段。
- 梳理总时间路径：HTTP accept → auth → schema → route → per-Agent lock → min-interval → Adapter spawn → first bytes → stream/completion → persist record → response.
- 检查何处使用 600s / 180s / 30s / 5s，各自计时起点和等待是否已经包含在总时间内。
- 检查现有调用记录字段与敏感信息保护，`store:false` 是否确实禁止所有相关持久化。
- 核对 Adapter CLI version/model list 证据，区分 `observed` 与 `inferred`，确认所有新增能力来自合法公开接口。

### 产出

一个简短的 `docs/architecture/RUNTIME_CONTRACT_GAP_AUDIT.md`，含既有实现清单、差距、接口基线 schema 样本（**全部使用合成 model/key**）及下面工作包的实际文件范围。不要大幅复述 `HANDOFF`。

**验收**：审计是事实表，不修改用户连接、配置或生产数据；Mock 当前行为可复现；不在扫描时自动触发真实生成。

---

## 3. M1 — 统一 Gateway Deadline、Queue 与 Cancel 语义（P0）

### 3.1 必须保持的原则

当前 Gateway 有多个独立期限：`REQUEST_TIMEOUT=600s`、`STREAM_IDLE_TIMEOUT=180s`、`AGENT_QUEUE_TIMEOUT=30s`、`AGENT_REQUEST_INTERVAL=5s`。这些限额合理与否要以数据判断，**不是简单都改成更大值**。

实施一套请求级 `ExecutionBudget`（可以是现有结构扩展）：

- 每条入站请求生成单调时钟的绝对 `overall_deadline`。
- auth/route/等待 Agent lane/最小启动间隔/Adapter 实际执行/响应编码都受**总预算**约束；子期限为上界，不能超过剩余预算。
- Queue 限时从等待 lane 开始，到拿到 lane 结束；interval sleep 与真实 Provider/CLI 执行分离统计，不错计入模型推理耗时。
- 非流和 SSE 均受完整请求期限；SSE 空闲超时独立处理，**发送心跳不能让无真实进展的请求无限延长总 deadline**。
- 客户端断线时取消本机正在执行的工作（若可控）并释放 Queue/Lane；区分“客户端停止接收”与“上游 Provider 停止推理/停止计费”，后者不可证明时标 UNKNOWN。
- 确保 deadline/cancel 后不会将迟到成功写入已结束请求记录，不能留永久锁或 zombie 子进程；进程组清理需要边界安全并避免误杀其他任务。
- Queue 超时、Adapter 运行超时、上游冷却、Key RPM、订阅额度分别有明确错误类别；保留现有 `LMX_*` 的稳定兼容性，必要时增量添加错误 `reason_code`，不任意破坏消费者已有错误分类。
- 不靠取消后盲重试去“修复”超时；无法证明上一次未执行时不自动重复发起可能计费的推理。

### 3.2 时间线监测字段

至少在**内部**可得到：

`accepted_at`, `auth_route_ms`, `queue_wait_ms`, `interval_wait_ms`, `adapter_startup_ms`, `first_output_ms`, `model_execution_ms`（如可区分）, `response_finalize_ms`, `total_elapsed_ms`, `cancel_requested`, `upstream_cancel_verified`.

所有 metric 可空；如果启动时间、第一文字时刻或远端执行阶段不可观测，则为空而非 0。尽量用单调时间，Unix 时间只用于可见审计时刻。

### 3.3 离线验收

使用虚拟时钟/受控 Async Adapter fixture 测试：

1. 队列 25s + 真实执行 10s，记录总耗时、排队耗时正确。
2. Queue 超过 30s 出错、`Retry-After` 在适用情况下保留，错误不冒称模型不存在。
3. 总预算已过，不能因 interval sleep 或心跳继续执行。
4. SSE 180s 无内容与 SSE 正常 heartbeat 的整体 deadline 区分。
5. 客户端断开、并发冲突、冷却、spawn 失败、内部异常无永久 lane。
6. 限时取消后远端无法验证停止，`upstream_cancel_verified=UNKNOWN`，不会声称节省真实订阅次数。

生产 600/180/30/5 默认值仅当测试证明需要变更时才调整，并记录迁移/后向兼容。`GET /api/health.execution_limits` 保持既有字段可读。

---

## 4. M2 — 统一 Versioned Capability Schema 与真实适配器能力（P0/P1）

### 4.1 不新建第二套目录

**沿用现有 `/api/chat/models` `schema_version=2`。** 默认做兼容补充，非破坏性扩展不轻易递增主版本；真正不兼容才单独版本演进且保留旧契约通道/协商路径。

每个**具体 model id + adapter + 已知 CLI/SDK 版本 + 已授权 connection scope** 发布：

- 有效能力 `effective_support`: `SUPPORTED | UNSUPPORTED | UNKNOWN`；
- `native_support`, `public_interface_support`, `adapter_state`;
- `limiting_layer`: `gateway | adapter | public_interface | model | unknown`;
- 依据：公开 `help/schema/version/list`、模拟 adapter 测试还是获用户授权的实际生成；
- `source`, `observed_at`, `version`, `live_verified`, `confidence/unknown_reason`（仅补当前缺失的非必填字段）；
- 明确的文本、messages roles、stateless history、native session resume、stream、json_object/json_schema、上下文/输出限制、temperature/top_p、reasoning_effort、usage/finish/cancellation 等能力；
- 公开来源变化后失效/过期机制；能力快照不能无限期冒充精确真值。

### 4.2 具体规则

- **格式支持分层**：CLI 自身 JSON event wrapper 不等于模型返回 JSON；`json_schema` 或 `json_object` 必须证明约束确实在指定公开接口生效；否则继续 `UNKNOWN/UNSUPPORTED`，标准请求仍可用文本模式。
- 消息序列：已支持“连续前置 system + 单 user 文本”，Codex 可具有 developer 层。**保留优先级**；不能用拼接所有历史到最后一个 user 的方式谎称支持 stateless history。
- `max_tokens/max_completion_tokens`：精确型号是否支持、上限和统计口径不可凭文档猜测；两个字段冲突仍返回 400。
- 不接受未知字段的严格 schema 保持现状；不能为了兼容至简添加诸如 `segment_ids`、`evidence_context`、`stage_name` 的 API 私有字段。
- **模型连通 vs 业务能力**：Mux 可执行通用 Tiny Smoke，验证授权、最小消息、文本正常、模式约束；不验证旅游 POI/转写校对能力，不给下游业务 `PASS`。
- 同一个 Adapter 各版本能力可以不同，同一家供应商不同型号能力可以不同；不扩大未知模型目录或把旧账号模型映射到新连接身份。
- 不得将响应模型自己声称的型号作为 `actual_model` 可信来源。只采纳 Adapter/上游可验证元数据。

### 4.3 验收

覆盖 1 个支持 System 的 Adapter、1 个不支持特定参数/Schema 的 Adapter、一个 UNKNOWN 上限型号以及模型目录变更：

- 能力目录仅列当前 Key 允许的模型；
- 不知道的能力不广告为 SUPPORTED；
- 已有 v2 客户端继续解析；
- 不会因一个参数不支持让整个模型从目录消失；
- unsupported reason 带 `param/limiting_layer/retryable=false`；
- 认证/角色/History/非 text 的拒绝不会静默降级；
- 任何真机 capability 升级须有单一精确型号的授权实测和可核对证据。

---

## 5. M3 — Runtime Telemetry、请求记录与用户可解释诊断（P0/P1）

### 5.1 后端监测范围

在现有调用记录基础上，扩展**无正文、低基数、大小有界**的字段：

| 字段 | 说明 |
| --- | --- |
| `request_id` | 由 Mux 生成/保留的公开安全请求关联 ID |
| `request_model` / `resolved_model` / `actual_model` | 请求别名、路由解析与真实报告型号区分 |
| `adapter_id` / `connection_id` | 只有当前 Key 可见或必要内部脱敏标识 |
| `route_kind` | 显式模型/虚拟路由、是否 fallback（如 Mux 确有其逻辑） |
| `queue_ms` / `interval_wait_ms` / `adapter_ms` / `total_ms` | 真正不同阶段的耗时 |
| `first_output_ms` | 仅在有证据时显示；stream 和 nonstream 分类 |
| `status` / `reason_code` / `http_status` | 通用生成终态与错误 |
| `usage_source` / `usage_scope` / `reported_*_tokens` | 实测与未知区别 |
| `finish_reason` / `finish_source` | 实际来源和结束原因是否可证明 |
| `normalization` | 已执行的**协议层**消息合并/字段规范化 |
| `response_bytes` / `request_bytes` | 本机实际读写字节（隐私与大小受控） |
| `body_recording_enabled` | 是否显式开启正文记录；本轮不自动改默认 |

现有调用记录默认只保存元数据，正文显式 opt-in；`store:false` 的调用不能通过“新 telemetry 表”绕过禁止持久化的用户意图。可选实时汇总要匿名聚合、不能暴露被拒绝客户端的受限型号或凭据。

### 5.2 成功与修复的边界

明确标注通用流程：

- `DIRECT_COMPLETED`：无需协议层降级、Adapter 正常返回可用完整结果；
- `PROTOCOL_NORMALIZED`：例如在明确支持规则下按原顺序合并前置 system 信息；必须列出发生的规范化，不暗中改变指令优先级；
- `ROUTE_FALLBACK`：由 Mux 真正执行的路由级回退，与客户端自己重新选模型不同；
- `PARTIAL/INCOMPLETE/ERROR/CANCELLED/UNKNOWN_TERMINATION`：任何没有证据的完成态不得伪装为 `stop`；
- **Mux 不实现** `JSON_DETERMINISTIC_REPAIRED`、`MODEL_REPAIRED` 等业务结果分类：它们由消费端依据模型返回内容作出。

`PROTOCOL_NORMALIZED` 可以与成功/失败共存；不可用单一字符串将二者混淆。

### 5.3 GUI

在已有本机调用记录、运行状态/连接详情页面增加最小信息闭环：

- 一次调用卡片能看见 `总耗时 / 排队 / Adapter / 模型 / 结果 / usage可信度`；
- 连接状态能区分 `ACTIVE/IDLE/BUSY/COOLDOWN/UNAVAILABLE`，且每个状态有真实来源与更新时间；
- 型号能力面板给出 `SUPPORTED/UNSUPPORTED/UNKNOWN` 和证据级别；不要将过期能力标成绿色正常；
- 文案区分“上游未提供用量”与“本次耗费 0 Token”，区分“连接成功”与“业务模型能力已验证”；
- 不新增用户必须填写的冗杂高级配置，保持软件“扫描→添加→验证→启用→复制 API 信息”的简易体验；
- 当前 UI 组件能承载时直接扩展，避免一次性重写全部 CSS。

### 5.4 验收

通过 mock 测试：现有 `store:false` 不会留下调用明细、路由事件或新持久 telemetry；对敏感错误及未知用量不泄露/瞎报；历史记录缺字段时 UI 可正常展示 `未知/旧版本`，不会崩溃；桌面窗口不同尺寸无阻断操作。

---

## 6. M4 — Response/Usage/实际模型契约完整性（P0）

沿用 `ChatResponse { id, model, choices, usage?, localmux }`、SSE `delta/usage/completed` 与 Responses 的现有设计。补齐以下不变式：

1. `usage` 只有上游明确报告且口径可判定时才填。`input/output/reasoning/cache` 如来源不同需带来源，不将 `session total` 当本次 `last` 使用。
2. Codex 的 `last` 与历史总量区分；Qoder/CodeBuddy 如果只提供累计 `model_usage`，不能直接用作 per-request；`unknown → null`。
3. `model`（公开兼容字段）、`localmux.resolved_model`、`localmux.actual_model` 的语义、来源与是否可空必须由文档锁定。
4. `finish_reason` 只在实际可判断时映射标准 `stop / length / content_filter`；拒绝、工具被禁止、超时、断流、取消的终态不允许固定伪写 `stop`。
5. `stream_options.include_usage=true` 的 SSE 末尾 usage 块保持标准兼容；流式中断不能制造完整消息。
6. `Chat` 的无状态 `messages` 与 `Responses.previous_response_id` 的原生续接 **严格分离**；一个不能代表另一个的 history 能力。
7. `localmux` 扩展 JSON 使用明确的 schema/稳定键集合和长度预算；不得复制原始 CLI 事件、Prompt 或 verbose 调试数据导致响应反而巨大。
8. 若追加 `X-Request-ID` 或阶段耗时响应头，必须**可选且兼容旧客户端**；不强迫现有下游传 `X-Client-Stage` 等私有头。
9. API 错误仍使用已有稳定 `LMX_*` 系列，并附 `request_id`、`param/reason_code/limiting_layer/retryable`（适用时）；凭据和上游原始错误正文需脱敏。

**验收**：标准 Python OpenAI SDK 与 TypeScript SDK 测试 `model list/chat/responses/nonstream/SSE/unknown usage/length/取消/400/429/503`；用量和 model 字段在跨 stream/nonstream 时满足同一含义，随机未报告的数值不会出现。

---

## 7. M5 — Adapter 模块可靠性与跨平台安全（P1）

### 7.1 按 Adapter 独立维护

每个 Adapter 按现有 SDK 约定提供相同维度：

- `discover`：只扫描安装/公开 CLI 程序路径，准确判定版本、平台和可用情况；
- `connect/verify`：以当前用户合法授权确认连接，不要求输入私有 token；
- `list_models`：精确型号目录，带更新时间与作用域；
- `capabilities`：元数据来源可信可回溯；
- `invoke`：仅传明确受支持的指令和参数；
- `cancel/cleanup`：有界停止、清理本机进程，不夸大远端计费保证；
- `status/usage`：AVAILABLE、BUSY、COOLDOWN、QUOTA_UNKNOWN 等，不制造“无限额度”；
- `failure mapping`：统一权限、队列、速率限制、额度、冷却、Provider 失败，但错误归因保留 Adapter 的实际证据。

### 7.2 本轮优先范围

- 先对已有 Codex App Server、Qoder CLI/Qoder CN、CodeBuddy CLI 做低风险一致性审计，修订单个 Adapter 的已知缺陷与 fixture。
- 对不同 Adapter 的真实 JSON Schema/结构化能力**保持未知**，直到逐型号实测核实具体 CLI/SDK 版本支持的公开机制。
- 保留当前禁用 tools/MCP、工作目录隔离、无项目文件访问的安全约束；有些 CLI 公开配置可禁用工具，也不能据此宣称供应商云端没有内部行为，实际运行需单独验收。
- macOS 与 Windows 的文件路径、权限、CLI 进程启动和停止要模块化；Windows 真机未实测不得标已支持。Tauri GUI 不拆成 Web 前端独立部署。
- 不为“支持更多平台”引入私人接口抓包、模拟付费 GUI 点击、复制登录 Cookie 或绕过订阅访问限制。

### 7.3 错误与恢复

- 本机 `Agent busy` 不能返回无关的 `MODEL_NOT_FOUND`；
- 不把未知订阅额度当作已耗尽或可用；
- 若本机断流，记录是否已启动 CLI、是否见到首字符、是否得到真实完成事件；无法证明时终态 `UNKNOWN`；
- 重启和升级保留明确批准的稳定 API Key/Scope/Connection，旧请求及记录完整，不自动扩大权限。

**验收**：对每类 Adapter 使用离线假 CLI/fixture 注入错误并验证资源清理；真实账号测试需用户显式选择一个准确型号，串行、有限次数执行。

---

## 8. M6 — 小规模通用回归基准与发布门禁（P0/P1）

### 8.1 Golden 的内容

**只测试通用网关能力，不测试至简业务。** 建议固定以下 Mock/Fixture：

| 组别 | 样本 | 必须证明 |
| --- | --- | --- |
| G1 | 目录、Key、权限边界 | 当前 Key 只能见其允许型号，schema v2 向后兼容 |
| G2 | 合法/非法消息角色 | system+user保真；history/developer/工具未实现时明确拒绝 |
| G3 | 参数范围和模型版本 | output/temperature/effort/JSON UNKNOWN/冲突拒绝一致 |
| G4 | direct vs protocol normalized | 不丢高优先级消息，不混淆成功与归一化 |
| G5 | 真实/未知 Usage & Model | last/total 不重复；unknown=null，实际模型不伪造 |
| G6 | stream 与 non-stream | 内容、终态、SSE usage 块、心跳及失败语义 |
| G7 | Queue / Timeout / Cancel | 时间总预算、忙碌、冷却、慢首字、断线、无遗留进程 |
| G8 | 调用记录及隐私 | 默认正文关闭、store=false绝不落持久追踪、敏感错误脱敏 |
| G9 | SDK 标准兼容 | Python/TypeScript 标准 SDK，不依赖定制字段 |
| G10 | GUI 关键路径 | 扫描→添加→验证→启动→复制标准接口信息及历史调用查看 |
| G11 | 升级/数据恢复 | API Key/作用域/连接不被静默更改，SQLite 完整性保留 |
| G12 | Windows/macOS（具备环境时） | 对当前可验证平台分别出证据，缺设备标 `NOT_VERIFIED` |

### 8.2 测量基线

可在 Mock/fixture 无真实额度前提下得到：

- `queue_wait_ms`、`adapter_startup_ms`、`total_ms`；
- `request_body_bytes`、`response_body_bytes`；
- `cancel_cleanup_ms`、lane 锁释放结果；
- 请求成功/错误/超时分类准确率；
- API Schema/SDK 回归通过率；
- 调用记录存储增长与普通请求开销，确保新增 Telemetry 不导致严重性能退化。

不对“真实订阅 Token 节省”作 Mux 独立承诺：下游业务消息内容不属于 Mux。HTTP 传输压缩可以作为后续实验，但它本身不减少模型输入 Token，也不可绕过请求/响应正文大小与安全边界。

### 8.3 执行

优先扩展 `scripts/verify.sh` 与现有 Rust/TS fixture，不额外引入第二套测试框架；实际命令以本仓库锁文件、工具链及现有验证脚本为准：

```bash
bash scripts/verify.sh
cargo clippy --workspace --all-targets -- -D warnings
```

官方 OpenAI Python/TypeScript SDK 集成应由 CI/隔离 fixture 重跑，报告精确版本及 pass/fail；历史的 105 Rust/7 frontend（或其他历史数字）**只作历史背景，不是本轮结果**。

### 8.4 发布门禁

**绝对不可放宽**：

- 未授权模型/Key/权限不得泄露或绕权；
- 旧标准客户端的 `/v1/models`、Chat、Responses 最小文本用法不得因本次变更失效；
- 不得把 `UNKNOWN` usage/model/finish/cancel 冒充已知；
- `store:false` 不能被新监测功能绕开；
- 请求超时和断线不能留下永久 Agent lane 或失控的本机子进程；
- 不得把“能访问 CLI”标成“全部 CLI 型号已真实推理通过”；
- 原有稳定凭据不可因升级自动重建、撤销或暴露。

---

## 9. M7 — 简化产品体验与 Adapter 扩展预备（P2，可独立选做）

这是此前产品方向讨论中的新增内容，**列入未来计划，但不自动进入 M0–M6 的实施范围**。

### 9.1 GUI 产品体验

单窗口主流程保持：

`自动扫描已安装应用 → 选择可用连接 → 验证连接与目录 → 显示模型/能力/额度可信度 → 启动本机 API → 复制 Base URL + Key + 示例请求`

建议在当前 UI 增量优化：

- 对“发现但未连接 / 连接可用 / 队列忙 / 已冷却 / 额度未知”的展示统一视觉语言；
- 只有用户已批准的范围才能获取模型列表或实际推理；
- 模型健康与业务模型正确性分开；
- 一键复制全部接口信息时保护 Key 显示和剪贴板风险：明确用户主动动作，不写日志；
- 无需分别解释所谓 free/paid 会员档，也不因为内部虚拟 `free` 路由去推断厂商某个订阅型号免费；
- 不替用户修改 Codex/Qoder/CodeBuddy 登录/计费设置。

### 9.2 Adapter SDK 发展

将 Adapter contract 写成清晰的独立规范：能力来源、调用生命周期、取消保证、目录缓存/TTL、与平台无关的进程抽象。后续 Kimi/GLM/豆包/其他 Agent 仅在具备**公开、用户授权、可验证本地接口**时新增 Adapter，逐个验收，不一次性“支持所有主流 Agent”但实际只是 UI 名称占位。

### 9.3 显式后续研究（当前不实现）

- 明确且可证明的 token cache / prefix reuse（不同 CLI 可能没有原生缓存元数据）；
- 基于公开 API 的标准结构化约束；
- 真正可证明的 stream cancel/remote cancellation；
- Windows ARM/x64 原生适配；
- GUI 中低开销的 token/额度统计，但必须保留 UNKNOWN；
- 多连接同 Adapter 的实例级身份/并发隔离（当前 lane 按 adapter id，后续不能错误共用）；
- 新 Adapter 的插拔扩展及沙箱权限审计。

每个后续议题需单独 Scope、验收和用户确认，不得以“为了至简 E2E”为由自动全量实施。

---

## 10. 与至简全链路的交接契约（**不是本仓库联调前置条件**）

至简是消费端，最终 E2E 由其自身 Agent 在独立计划中执行。Mux 单独实现完成后只需交付一份**无需泄漏用户信息**的协作清单：

1. Mux Git SHA / 正式编译包版本/OS/Adapter 精确已安装版本；
2. 标准 OpenAI endpoints 与 `/api/chat/models` 当前 schema 版本；
3. 各个样本型号的 `SUPPORTED / UNSUPPORTED / UNKNOWN` 能力来源/验证级别；
4. `GET /api/health.execution_limits` 的当前真实值；
5. request_id、错误格式、`localmux` 非必填字段的稳定含义；
6. 演示 Mock/SDK G1–G12 的报告；
7. 真实模型调用**已经获得用户批准的**最小测试结果（可为空；为空即 `NOT_VERIFIED`）；
8. 使用者需自行决定的超时与并发建议，但不侵入至简 Stage Policy。

**兼容策略**：

- 不增加新的强制请求字段；至简当前发送 `model + messages` 就应继续工作。
- `/api/chat/models` 保留已有 `schema_version=2` 兼容语义；新的能力来源字段均可选。
- 响应的 `localmux` 元数据可选、可部分缺失；业务调用方应在未知时保守处理。
- 追踪关联可使用已有 request ID、时间和公开范围内的模型信息；不要求对方植入私有协议。
- 不向至简提供裸 CLI Event/RPC，也不要求它探测 Adapter 私有 API。

### 联调发现缺陷的归属

| 失败现象 | 首要排查项目 |
| --- | --- |
| 模型列出但实际上未授权、队列卡住、Adapter 僵尸进程、标准响应无效 | Mux |
| 请求内容冗余、长 UUID、POI 误标、Evidence 无效、业务重试过多、笔记幻觉 | 至简 |
| 模型不能理解某旅游语境、业务 Golden 退化 | 至简选模/Prompt/模型自身；Mux 只核对已宣称的通用能力 |
| 上游账号额度/远端是否继续执行 | 证据不足标 UNKNOWN，不擅自归因或解密账号 |
| 两边请求时限不匹配 | 至简选总体任务预算；Mux公开自身单请求执行约束，协同诊断而不跨库修改 |

---

## 11. 实施提交顺序、独立关闭条件

| 工作包 | 优先级 | 单独关闭门禁 |
| --- | --- | --- |
| M0 当前事实与差距表 | P0 | 有基线样本、无用户数据变更 |
| M1 Deadline / Queue / Cancel | P0 | 虚拟时钟/断线矩阵全部有证据 |
| M2 能力目录/约束来源 | P0/P1 | schema v2 后向兼容；UNKNOWN 不假 PASS |
| M3 Runtime Telemetry / GUI | P0/P1 | 队列与实际执行可区分；store=false 和脱敏回归 |
| M4 Response/Usage/Model | P0 | Python/TS SDK、Chat/Responses/SSE 全绿 |
| M5 Adapter 可靠性 | P1 | 已有 Adapter mock/fixture 全绿；真实型号单独授权 |
| M6 通用 Golden/回归门禁 | P0/P1 | G1–G12 全部 PASS 或有明确环境阻断说明 |
| M7 简易桌面体验/未来 Adapter  | P2 | **只在用户另行授权时实施** |

一次 PR/提交只覆盖一个可审查工作包，必须标记需要/不需要数据库迁移，维护现有兼容接口和 CHANGELOG。不将“多个项目同时升级成功”作为 Mux 单仓测试的通过条件。

## 12. Agent 交付记录模板

```markdown
### 工作包：M?
- 分支 / 基线 HEAD / 完成 HEAD：
- 涉及的模块与不改动的边界：
- 旧机制复用点：
- Schema 兼容性（含 v2 旧客户端）：
- Deadline/queue/adapter/stream 关键测试：
- 连接权限与 store=false 回归：
- 实际模型、Usage、finish 的可信性：
- SDK/Rust/GUI 测试结果：
- 真实订阅调用：无/有，授权依据及型号：
- 操作系统与原生 App 验收范围：
- 残留 UNKNOWN / NOT_VERIFIED：
- 配置/数据库兼容及回滚：
- 文档更新：
- 是否达到单仓发布门禁：
```

### 核心决策

**Mux 只提供稳定、可观测、可正确拒绝的通用模型服务。短 ID、旅游证据、视频阶段、模型业务适用性及跨系统 Golden 均归消费端实现。两个 Agent 可以并行开发、分别验收，最后仅通过现有标准 API 进行受控联调。**
