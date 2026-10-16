# LocalMux / 智汇枢：桌面订阅统一 API 产品对齐与实施计划

> 文档类型：基于当前代码的评估 + 可交付 Codex/Agent 的增量实施计划  
> 日期：2026-09-28  
> 仓库：<https://github.com/ThunStorm/local-ai-mux>  
> 目标分支：`codex/phase0-main`  
> 评估基准提交：`e0a7e3556926db9c2c6b3d28849eea559c44013f`  
> 性质：**增量产品调整，不新建平行工程，不要求一次性支持所有应用**  
> 执行前提：先确认工作树、HEAD 与本计划基准的差异；对基准之后的改动先审阅、合并，禁止覆盖新代码。

## 0. 一句话愿景与成功定义

将用户已经安装、已经拥有授权的桌面 AI 应用，在可用且获准的官方本机运行时/CLI/SDK 接口边界内，包装成**本机 OpenAI 兼容 API 服务**。普通用户只需：

1. 安装并打开 LocalMux；自动扫描 Mac/Windows 中的 AI 软件。
2. 在应用卡片上点击「添加」；LocalMux 自动找到本机可用的官方接入途径，并分别验证运行时、登录、模型目录。
3. 在一个界面看到**连接状态 / 模型状态 / 额度状态**；点击一个模型可按用户确认发送极小的真实测试请求。
4. 点击「启动 API」；复制 **Base URL、LocalMux API Key、模型 ID、现成配置/示例**，即可交给其他 OpenAI SDK / 客户端使用。
5. 重启电脑或 LocalMux 后，无需重新填写路径或重复登录；只有上游会话过期时提示在原应用登录。

**本阶段不按 free/cheap/balanced/best 分类，不自动判定所谓免费模型，不把另一套付费 API Key 当作现有桌面订阅。** 用户优先选择真实模型 ID。额度只展示上游能证明的事实；未知就是未知。

## 1. 评估依据与已经完成的事实

### 1.1 基础已存在，禁止重写

- Tauri 2 + React/TypeScript + Rust workspace；macOS/Windows 发现引擎，SQLite，操作系统凭据库，嵌入式 Axum Gateway，托盘及自动启动开关。
- `crates/adapter-sdk` 已有 `Adapter::health/list_models/get_quota/invoke/invoke_stream/resume`；`crates/adapters/codex|qoder|codebuddy` 已有独立实现。`crates/adapter-sdk/src/cli.rs` 和 `provider_http.rs` 可保留为共享工具。
- Gateway 已支持 `GET /v1/models`、`POST /v1/chat/completions`、`POST /v1/responses`，Key 的哈希存储/撤销、限流、权限、文本 SSE；Mock 场景的官方 Python/TS SDK 测试已在交接和 CI 记录中通过。
- Codex 使用官方 App Server；Qoder 和 CodeBuddy 目前使用 headless CLI；MiMo 开放平台使用**用户另行提供**的 Key，和 MiMo Desktop 应用登记无关系。
- 已有 `added_applications` 与 `connections` 两张独立表。此区分在存储/安全层是正确的，不必合表；要改的是用户流程及更精确的关联。

### 1.2 具体缺口：按影响排列

| 编号 | 现状证据 | 用户影响 | 实施判断 |
|---|---|---|---|
| G01 | `apps/desktop/src-tauri/src/lib.rs` 的 `adapter_for()` 仅硬编码 Codex/Qoder/CodeBuddy；`add_detected_app()` 只写登记表 | 扫到桌面 App 不等于能连接；新增 Adapter 还要改桌面控制层 | 引入 Adapter Registry + Bridge Resolver，应用卡片一键引导 |
| G02 | `crates/adapter-sdk/src/lib.rs` 的 `NormalizedRequest` 仅有 `model`、`prompt`；`crates/gateway/src/lib.rs::chat` 仅取最后一条 user 文本 | 丢失 system/assistant/前序 user；多轮对话语义不可信 | **P0 修复**；协议明确支持/拒绝，不静默丢弃 |
| G03 | `health()` 多数仅检查二进制文件存在；添加时通常只调用模型列表 | UI 的“连接成功”不能证明登录、模型可调用或额度可用 | 拆分多维状态和可追踪诊断 |
| G04 | Codex 动态列模型；Qoder CLI 目录解析；CodeBuddy 仅 `config get model` 得到当前选中模型 | 无法完整罗列原软件提供的可用模型 | 优先官方目录能力；不完整明确标注 `PARTIAL` |
| G05 | 现有 `get_quota` 只能返回 `Option<QuotaSnapshot>`，Codex 解析优先 primary、备选 secondary | 多窗口额度无法同时展示，UI 易把窗口误当余额 | 引入多个 quota bucket，并保留兼容层 |
| G06 | API Key 只在创建时返回；当前 API 页仅显示地址+单独建 Key，模型在连接动作后临时展示 | 启动服务后仍需跨页面拼接配置；重启后不能查看已丢失的明文 Key | 安全地保存在 OS 凭据库，做一站式 API 配置页 |
| G07 | Routing 页仍以 free/cheap/balanced/best、收费开关和手填优先级为中心 | 与“傻瓜式直接使用订阅模型”的本阶段目标偏离 | 暂时从主界面和新手流程隐藏，保留旧数据/兼容 |
| G08 | `connections.id` 以 app ID 为主键，运行时也是 `adapter.id()`，目录路径靠 `provider/model` | 同一个 App 的多安装、多账号、多工作区不可扩展 | 引入稳定的 connection instance ID，平滑迁移旧数据 |
| G09 | README、部分 Adapter README、Qoder/CodeBuddy 文档仍包含旧阶段/“尚未安装 CLI”等叙述 | Agent 和用户容易把旧交接当当前实际状态 | 更新文档单一事实来源与支持矩阵 |
| G10 | CI 的 SDK smoke 主要验证 Mock，真实订阅请求尚未普遍完成 | 不能据 Mock 声称真实桌面订阅能输出 API | 分离离线测试 / 用户授权的真实验收 / 实机发布验收 |

代码引用：`apps/desktop/src-tauri/src/lib.rs`、`apps/desktop/src/App.tsx`、`crates/adapter-sdk/src/lib.rs`、`crates/gateway/src/lib.rs`、`crates/adapters/*/src/lib.rs`、`migrations/0003_connections.sql`、`docs/HANDOFF.md`，均位于上述基准分支。

### 1.3 与原计划的关系

现有 `docs/LOCALMUX_PROJECT_PLAN.md` 是旧版完整路线图；`AGENTS.md` 是开发操作入口；`docs/HANDOFF.md` 是实况交接。**本计划是下一阶段的产品对齐增量计划**。既有 Phase 0–8 已实现内容按代码保留，不再逐项重做；若与原计划“本阶段必须按 free 分类、复杂路由 UI、新增收费 Provider”等要求冲突，以本计划的当前发布范围为准，并在实施时写一条 ADR 记录范围调整。不要维护两个互相矛盾的实施进度。

## 2. 产品边界与不欺骗用户的显示规则

### 2.1 接入对象

- **Desktop Application**：发现的软件实体，如 WorkBuddy.app、Qoder.app、Codex/ChatGPT.app；含安装路径、应用版本、图标来源。
- **Bridge Runtime**：这台机器上为该应用提供可用能力的官方 CLI / App Server / SDK /受支持的本机接口。是否共享桌面登录态必须实测。
- **Connection**：某次经过用户确认的可用连接实例，指向 Runtime + 账号上下文；不只是“在磁盘上发现应用”。
- **Model**：上游为当前连接实际报告的模型/选择档位；未报告的模型不能假冒“全部可用”。
- **Quota Bucket**：上游提供的一个额度窗口/积分池，明确单位、来源、重置时间和更新时间。
- **LocalMux API Key**：本机入口 Key，只认证 LocalMux；**不等于供应商 Key，也不创造新的上游额度**。

### 2.2 使用方式边界

1. 优先官方、稳定、允许的应用接口；不窃取 token、复制浏览器 Cookie、解密 Keychain，不拦截加密流量，不靠私有 API/GUI 模拟调用来绕过供应商限制。
2. 扫描只读元数据；用户点击「添加/验证」后才运行上游程序；模型真实测试可能消耗额度，必须用户触发并说明。
3. 不承诺所有“桌面应用订阅”都可转换为 API；若无授权接入能力，仍显示已发现/已添加，但状态为“暂不支持 API 接入”。
4. 原厂订阅与额外 BYOK/API 计划分开标注。MiMo API Key 连接不得归入 MiMo Desktop 的“订阅已连接”。
5. 初期只支持本机 loopback `127.0.0.1:8317`，不开放 LAN/公网；不绕过上游并发、速率、条款、账号限制。
6. 文本能力优先；无工具/多模态/JSON schema 等能力时必须在能力清单里写明，并对不支持的输入返回显式错误。

## 3. 建议架构：每个应用一个 Adapter，但不是每个应用重写所有东西

### 3.1 架构结论

**每种桌面应用一个独立 Adapter crate/模块**，且 Adapter 对外实现统一契约。应用发现、用户引导、统一状态、请求/响应规范化、API Key、Gateway、配额快照、GUI 由公共层提供。CLI/SDK/App Server 的底层传输可共享。**不要把 `adapter_for()` 的 match 原样扩展成越来越长的总入口。**

```text
Tauri GUI
  ├─ Application Inventory（安装发现/去重/手动添加）
  ├─ Connection Orchestrator（一键添加、接入决策、授权、验证、刷新）
  ├─ Status / Model / Quota Store（一个快照统一供首页、模型页、API 页）
  └─ API Setup（本机地址、LocalMux Key、模型 ID、示例）
          │
Adapter Registry + Bridge Resolver（静态注册、按应用/OS/版本匹配）
          ├─ codex adapter → official App Server
          ├─ qoder adapter → official Agent SDK 或经验证的 CLI fallback
          ├─ codebuddy/workbuddy adapter → bundled/official CLI
          ├─ kimi / glm / other adapters → 逐个提供能力和真实验收
          └─ optional BYOK provider → 明确单独类别
          │
Normalized messages / capability / quota / error contracts
          │
Axum Gateway → /v1/models /v1/chat/completions /v1/responses
          │
SQLite metadata + OS credential store + redacted telemetry
```

### 3.2 Adapter Descriptor / Registry（建议类型，允许等价实现）

```rust
struct AdapterDescriptor {
    adapter_id: &'static str,
    application_ids: &'static [&'static str],
    display_name: &'static str,
    supported_platforms: &'static [Platform],
    bridge_kinds: &'static [BridgeKind], // AppServer, HeadlessCli, OfficialSdk, ProviderApi
    supports: AdapterCapabilities,
}

// 公共协调器的步骤，实际代码可拆 trait，避免巨型单 trait：
// discover_bridges(app) -> Vec<BridgeCandidate>
// preflight(candidate) -> Version/Runtime/Policy diagnostics
// check_auth(candidate) -> AuthStatus
// list_models(candidate) -> ModelCatalog { completeness, models, observed_at }
// get_quotas(candidate) -> Vec<QuotaBucket>
// invoke(...) / invoke_stream(...) / resume(...)
// diagnostics(candidate) -> safe, user-actionable errors
```

- Registry 以**编译期静态注册**为第一版：无远程下载可执行插件、无任意动态库加载，保持打包、安全与跨平台简单。后期新增应用时只增加 Adapter crate、Descriptor、检测 Manifest、fixtures、支持说明和注册条目。
- `discovery` 仅知道如何识别安装与候选 Runtime；Resolver 负责把某个 desktop app 和包内 CLI/外部 CLI/官方 SDK 关联；不要让 UI 或 Gateway 针对供应商写 `if id == ...`。
- 公共 `cli` 仅负责进程隔离、stdin/stdout JSONL、超时/取消/子进程组清理；SDK sidecar 只负责官方 SDK IPC；`provider_http` 只负责明确授权的 HTTP Provider。业务数据由专属 Adapter 解释。
- 严格分别记录 `catalog_supported`、`quota_supported`、`invoke_supported`、`subscription_auth_confirmed`、`tested_versions`、`limitations`，不能用 `health == Healthy` 推断全部能力。

### 3.3 多连接标识（不破坏原有 `codex/model`）

- 内部 `connection_id` 使用持久化 UUID/slug，唯一定位 app + runtime 路径 + 已授权账号上下文；Path 和 Bundle ID 必须规范化，扫描去重不依赖 UI index。
- `Adapter::id()` 可以保留为类型 ID，但注册表与路由改用 ConnectionInstance ID；`QuotaSnapshot` 和模型缓存按 connection ID 存储，不再都挤在 `codex` 等固定 ID。
- API 首版继续支持单实例历史 ID：`codex/<upstream-id>`；多个实例出现时新增稳定 `connection-slug/<upstream-id>`，旧 ID 只能在唯一匹配时继续解析，存在歧义返回明确错误，不悄悄切账号。
- 新 migration 添加表/列 + 扫描旧 `connections`、`quota_snapshots`、会话映射迁移；禁止修改已执行 migration 文件。备份/回滚脚本和迁移单测必须有。

## 4. 用户体验与信息架构

### 4.1 主导航（简化）

首页 / 应用与连接 / 模型 / API / 诊断与设置。原 Sessions 和高级路由隐藏在高级区，不删除已有数据。首次进入首页自动触发只读扫描并展示明确进度。图形界面是核心，终端命令只能出现在可折叠排障区。

### 4.2 应用卡片

每张卡片展示：应用名称/图标/版本/安装位置、接入方式（包内 CLI / 官方 CLI / 官方 SDK / 暂不支持）、连接状态、模型数量、额度摘要、最后检查时间、主操作「添加并验证 / 重新验证 / 修复连接」。

用户点击「添加并验证」的内部流程：

1. 对扫描结果去重并选择优先接入方式：已验证包内 Runtime → 已验证外部 Runtime → 官方 SDK → 指引手动安装；不能成功时允许切换接入方式但不让普通用户手填危险参数。
2. 检查 Runtime 可执行、实际版本、必需依赖；验证失败给出可理解的诊断，不把版本存在当登录有效。
3. 检查官方登录状态；未登录显示「在原软件登录后重试」，不要索取原厂 token。
4. 读取动态模型目录；明确 `COMPLETE/PARTIAL/UNKNOWN`，每个模型携带真实上游 ID/显示名称/能力/是否可选（未知属性留空）。
5. 尝试读取官方额度；不可用时状态 `UNKNOWN`，**不阻止其他步骤成功**。
6. 持久化连接后进入模型页，默认不发送消耗额度的模型请求；可选「测试此模型」再发极小请求。

失败可恢复：安装缺失、CLI 版本不兼容、账号过期、模型目录失败、额度未知、模型不可调用各自有独立状态和修复按钮。

### 4.3 明确状态设计

- 应用层：`NOT_FOUND` / `DETECTED` / `ADDED` / `RUNTIME_MISSING` / `UNSUPPORTED`。
- 连接层：`NOT_CONNECTED` / `CHECKING` / `AUTH_REQUIRED` / `CATALOG_READY` / `DEGRADED` / `OFFLINE` / `ERROR`；带 error_code/时间/可执行修复动作。
- 模型层：`REPORTED` / `VERIFIED` / `UNAVAILABLE` / `UNKNOWN`；`VERIFIED` 只能来自真实测试成功，非扫描或列表。
- 额度层：`REPORTED` / `STALE` / `UNKNOWN` / `EXHAUSTED`，各 bucket 独立；Unknown 不等于 0。
- 服务层：`STOPPED` / `STARTING` / `RUNNING` / `ERROR`；显示实际绑定端口、健康检查和活跃连接数。

**单一汇总规则**：一个连接的额度未知时，仍可显示“已连接 · 6 个模型 · 额度未知”；某个模型测试失败，只影响其模型状态，不能抹除其他已报告模型。

### 4.4 模型列表页面

- 以应用/连接分组，展示「官方显示名称、上游原始 ID、LocalMux ID、支持文本/流式/续接、目录完整度、验证状态、最后更新时间」。
- 提供搜索、按应用过滤、手动刷新和单个模型测试；清楚区分“官方列出”和“已经验证能调用”。
- 当供应商只报告某一个选中模型时，展示 `1 个已报告模型 · 目录可能不完整`，不得生成假完整列表。
- 模型变化时保留人工收藏的 ID，但下架/失效模型标为不可用；`/v1/models` 列出当前可路由、已由上游目录报告的模型，不纳入纯历史记录。

### 4.5 API 页面（一站式复制）

服务已启动时放在首屏：

- 状态：已运行 / `http://127.0.0.1:8317/v1` / 可用模型数 / 上一次健康检查。
- LocalMux API Key：默认自动配置一个最小权限本地 Key；明文在 OS 凭据库中保存，GUI 初始遮罩；用户点击「显示/复制」后才展示或复制；失去凭据时走显式重置和旧 Key 撤销。
- 模型下拉：**实际从 `/v1/models` 当前授权 Key 的可用结果获取**；默认优先前次选定的真实模型，没有则请用户选择，不制造一个万能 ID。
- 单独复制按钮：Base URL、API Key、Model ID、Endpoint、cURL、Python SDK、JavaScript SDK、JSON 配置块。
- JSON 配置示例：`{ "baseURL": "http://127.0.0.1:8317/v1", "apiKey": "<用户主动复制的 LocalMux Key>", "model": "codex/<当前真实模型ID>" }`。示例必须由实时值生成，不能写死当前的演示模型。
- 「测试 API」以当前 Key 调本机网关发送需要用户确认的短文本请求；返回状态、实际使用模型、耗时、错误修复提示；**不自动调用付费上游**。
- 默认在 Key 作用域中允许已连接文本推理（若是 Agent Runtime，用户必须在 GUI **明确**授权它的只读 Agent profile，不能静默绕过既有 `agent:invoke` 安全门槛）；创建流程由一段简单解释替代复杂权限表，高级设置另放折叠区。
- 没有可用模型时可以启动 API、复制地址/Key，但清楚显示“暂无可调用模型”。不写“服务已就绪，可以聊天”。

## 5. API 兼容性必须先修的 P0

### 5.1 多轮语义

当前 `chat()` 找最后一个 user 消息并传给 `NormalizedRequest.prompt`，会静默丢弃先前的 system/user/assistant。新增 typed messages：

```rust
struct NormalizedMessage { role: Role, content: String }
struct NormalizedRequest {
    model: String,
    messages: Vec<NormalizedMessage>,
    // 可选：temperature、max_output_tokens、response_format 等能力字段；
    // 只对已支持的字段透传，不支持的参数必须在 API 层明确拒绝/说明。
}
```

- Gateway 不能丢 system、历史 user/assistant；Provider HTTP Adapter 必须按顺序发送完整 messages。
- Codex/Qoder/CodeBuddy 若官方协议不支持 chat role，可由专属 Adapter 实现**明确、经过测试**的等价上下文封装/原生会话策略；如果无法保留语义，则返回标准错误 `LMX_UNSUPPORTED_MESSAGES`，而非“200 成功但忽略指令”。
- Chat Completions 无 `previous_response_id` 时，每次请求依赖客户端传入的完整 messages；Responses 的原生 session 续接保持现有设计。禁止两个流程暗中混用状态。
- 当前 `usage` 固定 0 不得解释为真实 token 消耗；能获取真实上游统计就填，缺失就按兼容要求保守返回并在 GUI 标注“不提供上游用量”。

### 5.2 模型及流式验证

- `/v1/models` 返回的是**当前 Key 有权限 + 当前连接真实报告 + 当前可路由**的模型，而不是扫描到 App 后就生成虚假条目。
- `GET /v1/models`、Chat 非流式/SSE、Responses 非流式/SSE、取消/超时/断流/401/403/404/429 用官方 Python/TS SDK 回归。
- 保留已有错误模型、Key scope、请求体限制、会话 Key 归属、只读沙箱；新代码不能削弱安全测试。
- 功能范围清晰标注「OpenAI 兼容**文本子集**」，不宣称工具调用、图像、音频、structured outputs 已兼容。

## 6. 额度：从单值变多桶，且只报告可靠证据

建议 `QuotaBucket { connection_id, bucket_id, label, used?, limit?, remaining?, unit, window_minutes?, reset_at?, observed_at, source, status }`；支持“一号多额度”：例如 5 小时/周窗口、套餐积分/加购积分、组织配额。不要跨不同单位和重置窗口相加成“总余额”。

- Codex：从已有官方 `account/rateLimits/read` 扩展解析 primary + secondary 并列输出，保留 `ordinaryUsageAllowed` 限制；这叫“用量窗口”，**不是 Credits 余额**。
- Qoder：优先验证官方 Agent SDK `getUsageInfo()` / `get_usage_info()`；其 `userQuota`、`addOnQuota`、`orgResourcePackage` 应分别显示。SDK 值为空/缺字段时 `UNKNOWN`，不得把 session 用量倒算账户余额。
- WorkBuddy/CodeBuddy：先验证有没有可靠官方机器可读额度契约；没有就 `UNKNOWN` + 官方额度入口，绝不通过非公开接口或截图 OCR 自动抓取。
- MiMo API：平台余额单独归属 BYOK/API 连接，不与 Desktop 订阅混算。
- `source` 细分 `OFFICIAL_API` / `OFFICIAL_SDK` / `OFFICIAL_CLI` / `UNKNOWN`；可在内部兼容旧 `PROVIDER_REPORTED`。保留快照 TTL、错误诊断和每连接刷新节流，只有经过验证的值入官方额度区域。
- 后台按已有 5 分钟节奏刷新即可；失败不得清零、不得永远显示旧值为实时，前端显示“过期/上次更新”。

官方参考：
- Qoder 官方模型和额度 SDK：<https://docs.qoder.com/cli/sdk/model-policy>、<https://docs.qoder.com/cli/sdk/cost-usage>、<https://docs.qoder.com/cli/sdk/references-typescript>。
- Qoder CLI 模型说明：<https://docs.qoder.com/cli/model>；实际型号变化以用户账号动态返回为准，不能把文档示例硬编码成用户可用模型。

## 7. Adapter 具体演进优先级

| 阶段 | Adapter | 首批目标 | 容易误判的地方 | 准入条件 |
|---|---|---|---|---|
| A | Codex | 完整连接流程、动态模型、双额度窗口、真实请求验收 | CLI 文件存在≠登录有效；用量%≠积分余额 | 官方 App Server 实机成功+Python/TS SDK 实际调用 |
| B | Qoder | 官方 SDK 模型与额度；现有 CLI 作为经验证 fallback | SDK 的授权可能不同于桌面登录；CLI 输出格式需版本测试 | 实机模型/额度/短调用三项证据 |
| C | WorkBuddy/CodeBuddy | 自动发现包内 CLI、依赖预检、可报告目录与实测 | 桌面包中有脚本≠无需 Node/共享登录；当前仅 selected model | 真实调用和目录完整度标注通过 |
| D | 其他应用 | Kimi、GLM、TRAE、MiMo Desktop、Ollama 等逐个补 | 探测到图标不等于已有官方订阅桥；BYOK≠Desktop | 每个单独研究、证据、明确支持状态 |

具体规则：

1. Codex：保留现有 `crates/adapters/codex`，新增 health/auth/connection diagnostics、双窗口 quota、真实 smoke；不要重复造网关。
2. Qoder：评估官方 `@qoder-ai/qoder-agent-sdk` 是否允许使用当前用户已授权 CLI 身份（官方文档有 `qodercliAuth()` 示例）；若需要受支持的 SDK sidecar，则用固定版本、签名/哈希校验、随桌面包交付并提供 IPC；不得要求终端运行 Node 服务。比较 API 稳定性、包许可/分发与启动成本后决定主/备路径。优先 `getAvailableModels()`/`getUsageInfo()`；不依赖文档演示型号。
3. CodeBuddy：复用当前官方 headless CLI，增加 Node 运行时检查和包内 CLI 解析。**完整模型列表若没有官方稳定机器接口，就保持部分目录，并记为待支持，不读取可能包含 API Key 的原厂私有配置。**
4. Kimi/GLM 等：已存在的 Unsupported/隔离模块按实际官方可授权能力迭代。新 Adapter 必须有最小 fixture 和真实验收，不允许“加一个空目录+图标就算已支持”。
5. MiMo Desktop 与 MiMo API、Ollama 本地模型和桌面订阅是不同连接类别，UI 标注清楚。用户本阶段不要求 free 分类，也不因为某模型可本地运行就归为订阅模型。

## 8. 可直接执行的工作包（按顺序，一个工作包一个可验收提交）

### WP0 — 固定基线与冲突消除（不扩张功能）

**主要文件**：`AGENTS.md`、`docs/HANDOFF.md`、`README*.md`、`docs/product/*`、`docs/adapters/*`、`CHANGELOG.md`、新增 `docs/architecture/adr/XXXX-product-focus.md`。

- [ ] `git status` + `git rev-parse HEAD`，先了解工作树已有未提交内容，基准变化只审阅相关文件。
- [ ] 编译与跑现有 `scripts/verify.sh` 或 Windows 对应脚本，记录本机/CI/真实上游三类证据；不能借用旧 CI 当作当前工作树结果。
- [ ] 补 ADR：本阶段真实模型优先、隐藏免费路由、保持凭据和只读边界。
- [ ] 更新陈旧 README/Adapter 安装描述；补可查验的支持矩阵字段（发现/连接/目录/额度/真实调用/平台）。

**验收**：旧迁移和 Mock 兼容无回归；文档没有相互冲突的当前状态。

### WP1 — Chat 多消息修复（最高优先级）

**主要文件**：`crates/api-types/*`、`crates/adapter-sdk/src/lib.rs`、`crates/gateway/src/lib.rs`、`crates/gateway/src/responses.rs`、`crates/adapters/*`、`crates/gateway/tests/*`。

- [ ] 引入结构化 message 请求；逐 Adapter 显式实现或拒绝角色语义。
- [ ] HTTP provider 保留完整顺序；Codex/Qoder/CodeBuddy 为 system + 历史信息建立有测试的转换边界。
- [ ] 新增 system 历史不丢失、模型不支持时 4xx、流式与非流式一致性测试。
- [ ] 不扩展未支持的工具、多模态或任意 Agent 写权限。

**验收**：3 轮 `system → user → assistant → user` 的内容能被实际转发/可证明保留，或明确返回“不支持”，绝不静默忽略。

### WP2 — Adapter Registry / 多维状态契约

**主要文件**：`crates/adapter-sdk/*`、`crates/domain/*`、新增 `crates/connection-manager/`（如确有必要）或并入现有 core，`apps/desktop/src-tauri/src/*`、`migrations/0009_*.sql`。

- [ ] 静态注册 Adapter Descriptor，迁走 `adapter_for()` 的硬编码中心；保留旧连接恢复路径。
- [ ] 将 `discover/preflight/check_auth/catalog/quotas/diagnostics` 组织为公共生命周期；每次失败带步骤、可行动错误码和安全日志。
- [ ] 引入稳定 connection instance；过渡期旧 ID 不破坏；测试同 App 两个实例不互相注销或共用额度。
- [ ] 连接持久化改为验证后的原子流程；回滚 credential/DB，避免数据库显示已连但运行时不可用。

**验收**：添加一个模拟 Adapter 仅改其模块+Registry/Manifest，不必改 Gateway/GUI 多处供应商分支；旧数据库升级与恢复测试通过。

### WP3 — 扫描到一键添加闭环

**主要文件**：`crates/discovery/*`、`apps/desktop/src-tauri/src/*`、`apps/desktop/src/*`、相应 discovery fixtures。

- [ ] 将同一应用包、包内 CLI、PATH CLI 关联为一张用户卡片（保留多个运行时供高级选择）；版本和路径真实可追溯。
- [ ] 一键添加向导自动处理 Runtime 预检、授权提示、目录、额度，并在局部失败时保持其余成功状态。
- [ ] 支持手动选择本机 App 路径，但不得直接执行任意上传的 shell 命令；路径再扫描、签名/发行者（若可用）及白名单策略由 Adapter 验证。
- [ ] 添加/移除/重扫幂等；重扫不会误删用户连接和 Key。

**验收**：Codex 与 WorkBuddy 包内 CLI 不重复展示为互不关联的“软件”；仅桌面存在但桥缺失时给出下一步，而非伪造连接。

### WP4 — Model Catalog 与模型验证

**主要文件**：`crates/domain/*`、`crates/adapter-sdk/*`、`crates/gateway/*`、`apps/desktop/src/*`、catalog migrations/fixtures。

- [ ] 目录对象加入 `completeness`、来源、获取时间、可选状态、官方模型 ID/显示名、能力声明与实测状态。
- [ ] 动态刷新/失效、并发去重；不允许把旧缓存当当前真实目录。
- [ ] 用户点击单模型验证才发送短请求；成功 `VERIFIED`，失败分类并记录不含提示词的摘要。
- [ ] `/v1/models` 只返回当前已连接/当前 Key 可调用的模型；已报告未实测可标注后台状态，但不伪装为已验证。

**验收**：Codex 动态型号反映实时目录；CodeBuddy 部分目录清楚标注，不生成未报告型号；Key 限制下列表不泄漏其他连接模型。

### WP5 — 额度多桶及三应用真实接入

**主要文件**：`crates/quota/*`、`crates/adapters/{codex,qoder,codebuddy}/*`、`apps/desktop/src/*`、`docs/adapters/*`。

- [ ] 兼容旧单快照 schema，新表保存多 bucket；旧数据迁移后保留来源和刷新时间。
- [ ] Codex primary/secondary 同时显示；Qoder SDK 官方模型/额度能力验证并确定 SDK/CLI 主备；CodeBuddy 检查官方接口，无契约则 Unknown。
- [ ] 模型和额度刷新节流、过期显示、认证失败与真实 quota exhausted 分离。
- [ ] 真人在 GUI 点击显式连接后分别完成 Codex/Qoder/WorkBuddy 真实请求（可以逐个发布支持标识）。

**验收**：可观测 5h/周等独立窗口而不错误相加；Qoder 返回的不同额度池独立显示；用户无 Qoder SDK/登录时自动降级或提供明确操作，不悄悄假设登录共享。

### WP6 — API 一站式复制与首启默认 Key

**主要文件**：`apps/desktop/src/App.tsx`（实施时按模块拆分，不再增大巨型文件）、`apps/desktop/src-tauri/src/*`、`crates/security/*`、`crates/secrets/*`、`crates/storage/*`。

- [ ] 第一次开启 API 时初始化一个稳定最小权限 LocalMux Key，哈希在 SQLite、明文仅在 OS vault；幂等，重启后 GUI 必须能显式显示/复制。
- [ ] OS vault 丢失/拒绝访问时不得假装 Key 可用：显式创建新 Key、原 Key 撤销；绝不在日志/错误/设置明文落盘。
- [ ] Agent 类调用仍遵守 `agent:invoke` 与 GUI 显式只读授权；向导显示一句普通人能理解的提示。
- [ ] 用真实绑定端口、授权后的模型数据生成 Base URL、Key、Model ID、cURL、Python/TS/JSON；一键单项复制和整套配置复制（含 Key 需要明确点击）。
- [ ] 服务未启动/端口占用/没有模型/Key 失效分别显示修复建议。初期固定 loopback，不开放公网功能。

**验收**：新用户首启从扫描到复制配置不需要终端、不手写 JSON；重新启动后可安全复制已有 Key；旧 Key/已有 scope/撤销测试保持通过。

### WP7 — UI 收敛与 free 逻辑冻结

**主要文件**：`apps/desktop/src/*`、`docs/product/UX_SPEC.md`、原路由测试。

- [ ] 主导航以应用、模型、API 为中心；按视觉语义统一状态徽标、按钮尺寸、字体及紧凑信息密度。
- [ ] free/cheap/balanced/best 的控件、营销式“最佳模型”展示和初次引导移出主界面；**不删除内部旧路由实现、旧数据或旧 API Key 权限**，暂时作为旧配置/高级兼容。
- [ ] 新生成的默认配置只用具体模型 ID；旧客户若显式请求已有虚拟路由，保持既有可解释行为或提供迁移提示，不偷偷改路由。
- [ ] 分拆 `App.tsx` 和单文件 Tauri commands 成 features/components/commands，避免继续在两个巨型文件叠加分支；保持数据流统一。

**验收**：普通用户无需理解额度计费分类或优先级即可连接一个 App 并复制模型配置；原回归测试继续通过。

### WP8 — 实机验收 / 打包 / 文档封版

**主要文件**：`.github/workflows/*`、`tests/compatibility/*`、`docs/HANDOFF.md`、`docs/product/SUPPORTED_APPS.md`、`CHANGELOG.md`、`README*.md`。

- [ ] macOS 与 Windows 原生 CI：Rust fmt/clippy/test，前端类型/测试/构建，原生安装包构建。
- [ ] Mock 官方 SDK smoke + 用户明确授权的真实 Codex/Qoder/WorkBuddy 文本非流式/SSE 回归；真实测试与 CI Mock 结果分开记录。
- [ ] 安装/升级/关闭到托盘/重启恢复/本机 Keychain-Credential Manager 实际读写/卸载数据策略检查。
- [ ] 按 `AGENTS.md` 原闭环执行本机部署/重启，不能覆盖运行中任务；更新 `HANDOFF` 实际通过/未通过项及构建来源。
- [ ] 交付一个给普通用户看的 README 和一个给开发者看的 Adapter 增加模板；不要重复维护几套不同的支持矩阵。

**验收**：真实订阅支持范围有证据；Windows 没实机验收则标注未验收，不宣布正式全平台发布。所有现有安全回归及 Mock SDK 测试通过。

## 9. 新增 Adapter 的标准作业（后续扩展模板）

每加一个应用，严格提交以下内容：

```text
crates/adapters/<app>/
  Cargo.toml
  src/lib.rs                # Adapter 实现 + Descriptor
  tests/fixtures/           # 官方版本的脱敏协议样例
  tests/protocol.rs         # 目录 / 授权 / SSE / 错误 / 取消
crates/discovery/...        # 该 App 的静态 Manifest/关联规则
apps/desktop/...            # 仅共用卡片数据，不写供应商专属大分支
docs/adapters/<APP>.md      # 接入方式、版本、Auth、模型、额度、边界、真实验收
```

检查清单：

- [ ] 检测不触发登录、不读取私密凭据；运行时来源和版本可证。
- [ ] 用户显式授权 + auth 诊断 + 可读的修复指引。
- [ ] 模型 ID 动态列表，不能枚举的说明 `PARTIAL/UNKNOWN`。
- [ ] 额度按官方机器接口返回；没有则 Unknown；不把本地统计当剩余额度。
- [ ] 只读文本调用、增量流、取消/超时/错误映射；无法保障安全则不曝光该能力。
- [ ] 同一 app 多连接/重启/CLI 消失/版本变更的行为测试。
- [ ] 官方 Python/TS SDK 调 LocalMux Gateway 的端到端测试；真实上游验收记录带版本与日期。
- [ ] 更新支持矩阵与 HANDOFF，编译测试并做相应平台安装验收。

## 10. 验收矩阵：禁止“扫描成功=功能完成”

| 场景 | 预期 |
|---|---|
| 仅安装 Qoder Desktop，无获支持 Runtime | 可登记；显示桥缺失/指引；不得显示已可 API 调用 |
| 已装 Codex CLI，但未登录 | 检测到 → 添加时提示在原 App 登录 → 不注册可路由模型 |
| 已登录 Codex，目录返回多个模型 | 显示原厂报告的列表+LocalMux ID；单模型可手动测试 |
| WorkBuddy 包内 CLI 依赖缺失 | 指明运行时依赖，允许一键修复方案但不静默装危险来源 |
| CodeBuddy 只能得当前选中模型 | 展示该模型，目录标 `PARTIAL`，不声称所有模型 |
| Quota 接口不可用 | 连接/模型可用仍保持；额度显示未知、来源和时间 |
| Codex primary/secondary 双窗口 | 同时显示剩余比例/重置时间，不混成积分余额 |
| Chat 传入 system+历史消息 | 全部被正确处理，或明确拒绝；不得只取最后 user |
| `/v1/models` 请求无 Key/已撤销 Key | 正确 401/403，不泄漏连接/模型目录 |
| API 端口占用 | UI 显示端口占用、服务未启动；不返回旧地址假成功 |
| Key 重启后再次复制 | OS 凭据可安全取回；vault 缺失则提示重建并撤销旧 Key |
| 同 App 两个连接 | 模型目录、额度、会话归属互不串用；旧 ID 歧义显式报错 |
| 上游 429、网络失败、CLI 崩溃 | 各自准确错误；恢复后可刷新重试，不能误报 Key 失效 |
| Windows/.app 安装包 | UI、扫描、连接恢复、凭据和 Gateway 原生测试记录 |

## 11. 执行约束与防过度开发

- **第一优先** WP1（兼容性）+ WP2/3（加 App 闭环）+ WP6（API 复制）。在它们完成前，不要为所有国内外应用批量创建空 Adapter。
- 不新建另一个前端 Web 站、不使用 CLI-only 发布、不推倒 Tauri/Rust、SQLite、已有 Key/Session/Gateway；真实 Adapter 要增量替换或扩展。
- 原计划中的 free 分类、复杂自动路由、长期记忆、任意 Agent 工具执行、跨机远程公网访问、云端账号共享、自动抓取网页积分不属于本轮范围。
- 不允许**仅靠造一个模拟模型**完成真实验收；CI Mock 测试与真实用户授权测试严格分开。
- 不写一次性“全局兼容模式”来掩盖供应商差异；每个 Adapter 能力、版本和错误路径均可诊断。
- 不以用户的任何真实 token、cookie、供应商 API Key 写 fixture、日志、截图或示例。

## 12. 给 Codex/Agent 的直接执行提示词

> 你正在 `ThunStorm/local-ai-mux` 的 `codex/phase0-main` 分支继续开发。先读 `AGENTS.md`、`docs/HANDOFF.md`，确认当前 HEAD 和工作树，随后读本计划及本工作包涉及的源码；禁止全库泛扫描和重建项目。按 WP0→WP8 顺序逐包实施，首次运行测试建立基线。当前产品范围是“已安装桌面应用→官方接入→真实模型与额度→OpenAI 兼容 API 一键复制”，暂不做 free/cheap/balanced/best 分类。优先修复 Chat 历史消息丢失；随后做静态 Adapter Registry、连接生命周期、完整度明确的模型目录、可验证额度、多维状态、一键 API 配置。不得读取或复制上游私有凭据，不用未授权私有接口；无官方入口时标 unsupported/unknown。每包更新相应测试、文档、CHANGELOG、HANDOFF，完成本机安装验证并记录真实/Mock/平台验收边界。若遇新版上游接口与计划不同，先给出证据再调整该 Adapter，不静默改整个架构。未经用户要求不要提交推送到远程分支。

---

### 关键仓库来源（评估基准版本）

- [交接记录](https://github.com/ThunStorm/local-ai-mux/blob/e0a7e3556926db9c2c6b3d28849eea559c44013f/docs/HANDOFF.md)
- [Agent 约束](https://github.com/ThunStorm/local-ai-mux/blob/e0a7e3556926db9c2c6b3d28849eea559c44013f/AGENTS.md)
- [Tauri 命令与连接流程](https://github.com/ThunStorm/local-ai-mux/blob/e0a7e3556926db9c2c6b3d28849eea559c44013f/apps/desktop/src-tauri/src/lib.rs)
- [前端页面](https://github.com/ThunStorm/local-ai-mux/blob/e0a7e3556926db9c2c6b3d28849eea559c44013f/apps/desktop/src/App.tsx)
- [Adapter SDK](https://github.com/ThunStorm/local-ai-mux/blob/e0a7e3556926db9c2c6b3d28849eea559c44013f/crates/adapter-sdk/src/lib.rs)
- [Gateway](https://github.com/ThunStorm/local-ai-mux/blob/e0a7e3556926db9c2c6b3d28849eea559c44013f/crates/gateway/src/lib.rs)
- [Codex Adapter](https://github.com/ThunStorm/local-ai-mux/blob/e0a7e3556926db9c2c6b3d28849eea559c44013f/crates/adapters/codex/src/lib.rs)
- [Qoder Adapter](https://github.com/ThunStorm/local-ai-mux/blob/e0a7e3556926db9c2c6b3d28849eea559c44013f/crates/adapters/qoder/src/lib.rs)
- [CodeBuddy Adapter](https://github.com/ThunStorm/local-ai-mux/blob/e0a7e3556926db9c2c6b3d28849eea559c44013f/crates/adapters/codebuddy/src/lib.rs)
- [Qoder 官方 SDK Models](https://docs.qoder.com/cli/sdk/model-policy) / [Quota](https://docs.qoder.com/cli/sdk/cost-usage)
