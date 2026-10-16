# LocalMux / 智汇枢 — 跨平台桌面 Agent 订阅统一 API 网关实施计划

> 文档类型：`PROJECT_PLAN.md` / Agent 可直接执行实施计划
> 当前状态：从零开始（Greenfield）
> 目标平台：macOS + Windows
> 产品形态：原生安装桌面应用（GUI + 托盘/菜单栏 + 内嵌本地 API 服务），**不是命令行产品，也不是浏览器 Web 管理页**
> 默认网络边界：仅监听 `127.0.0.1`
> 核心协议：OpenAI-compatible API；后续可扩展 Anthropic-compatible / MCP / ACP
> 核心原则：本地优先、显式授权、动态发现、Adapter 解耦、订阅/免费档感知、Session 可持续、可观测、可维护

---

## 0. 给执行 Agent 的硬性指令

本文件是首个实施阶段的最高优先级工程约束。执行 Agent 必须遵循以下规则：

1. **不要把项目做成“若干 Python/Node 脚本 + Web Dashboard”。** 最终必须能打包为 macOS `.app/.dmg` 与 Windows `.exe/.msi` 或 NSIS 安装包，并以独立桌面窗口运行。
2. **不要通过模拟鼠标、OCR 或操控第三方 GUI 作为主要接入方式。** 优先使用官方/稳定的 CLI headless、SDK、App Server、ACP、公开兼容 API 或 documented local runtime。
3. **不要秘密读取系统钥匙串或第三方应用私有 Token。** 自动扫描只用于发现应用、版本、可执行文件、公开配置和可接入能力。读取/复用认证必须由对应 Adapter 明确实现，并经过用户“添加/连接”动作。
4. **不要逆向并硬编码厂商私有接口作为 P0 主链路。** 若某 Provider 只有不稳定私有接口，则标记为 `experimental`，不得阻塞正式版主路径。
5. **不要把 Agent、Provider、Model、Account 四个概念混为一个。** 它们必须在领域模型中分离。
6. **不要写死“免费模型名单”。** 免费/0-credit/Reserve 必须由 Adapter 动态发现或明确上报；无法确认时标记 `UNKNOWN`，不得假装免费。
7. **不要把长期 Memory 与 Provider Session 混为一个东西。** V0.1 先解决 Provider Session / Conversation Continuity；长期语义记忆放到后续阶段。
8. **任何新增 Adapter 必须遵守统一 Adapter SDK 契约，不允许在 Router/GUI 中出现大量 `if provider == ...`。**
9. **每个实现阶段都必须补齐对应测试与文档。** 行为变更未更新文档视为未完成。
10. **不得大面积扫描仓库。** 先阅读根目录 `AGENTS.md`，再按 Documentation Map 进入对应模块。
11. 每个 Phase 完成后必须：
    - 编译通过；
    - 单元测试通过；
    - 相关集成测试通过；
    - 文档同步；
    - 更新 `CHANGELOG.md` 的 `Unreleased`；
    - 输出本阶段变更摘要与未解决问题。
12. 如果某第三方 Agent/Provider 当前无法稳定实现，**先完成接口、检测、Capability/Unsupported 状态和 Mock 测试，不得用脆弱方式强行“实现成功”。**

---

# 1. 产品命名

## 1.1 推荐工作名

### 英文：`LocalMux`
### 中文：`智汇枢`

含义：

- `Local`：本地优先、运行于用户自己的 Windows/macOS 设备；
- `Mux`：Multiplexer，多路复用器，将多个 Agent、订阅和模型统一汇入一套接口；
- `智汇枢`：强调“AI 能力汇聚 + 路由枢纽”，不把产品限制在单一模型或单一 Agent。

当前实施阶段使用以下可替换标识：

```text
Product Name: LocalMux
Chinese Name: 智汇枢
Internal Slug: localmux
Default API Prefix: lmx_
Default Port: 8317
```

**重要：品牌名必须可替换。** Bundle ID、Cargo package、前端显示名、安装包名、数据库目录必须通过集中配置定义，不得散落硬编码。

建议默认：

```text
macOS Bundle ID: app.localmux.desktop
Windows App ID: LocalMux.Desktop
Data directory:
  macOS: ~/Library/Application Support/LocalMux/
  Windows: %APPDATA%\LocalMux\
```

正式发布前必须做商标、域名、GitHub organization、应用商店名称的独立核验；当前命名仅作为工程工作名。

## 1.2 备选名称

1. `LocalMux / 智汇枢` — 最推荐，技术含义明确，适合本地桌面网关。
2. `AgentSwitch / 智汇交换台` — 更强调路由，但英文通用度较高。
3. `AgentPatch / 智接台` — 有“Patch Bay/接线台”隐喻，但容易被理解为代码补丁工具。
4. `ModelHub Local / 模型汇` — 容易理解，但产品实际还包含 Agent/Session，不够完整。
5. `AI Switchboard / 智能交换台` — 产品定位准确，但品牌感较弱。

---

# 2. 产品愿景

LocalMux 是一个 Windows/macOS 跨平台桌面应用，用于：

> 自动发现本机已安装的 AI Coding Agent / AI Agent 与可复用的订阅能力，在用户显式授权后，将这些能力标准化为统一的本地 Runtime，并通过一个 OpenAI-compatible Base URL + 本地 API Key 对外提供服务。

典型用户已经购买或安装：

- ChatGPT/Codex；
- Qoder；
- WorkBuddy / CodeBuddy；
- Kimi Code；
- Claude Code；
- GLM Coding Plan / ZCode；
- 豆包 / 火山方舟 Coding/Agent Plan；
- Gemini CLI；
- OpenCode；
- Qwen Code；
- TRAE / Cursor / Cline / Roo / Kilo 等客户端或 Agent。

用户不希望重复购买 API Token，而希望在厂商允许的边界内复用自己已经拥有的本地登录、套餐、订阅或 Coding Plan 能力，并统一提供：

```text
Base URL: http://127.0.0.1:8317/v1
API Key:  lmx_xxxxxxxxxxxxxxxxx
```

客户端只需要选择模型：

```text
free
balanced
best
codex/...
qoder/...
workbuddy/...
kimi/...
glm/...
ark/...
```

---

# 3. 非目标（必须明确）

V0.x 不做：

- 云端 SaaS 账号系统；
- 出售或转售第三方厂商套餐；
- 将个人订阅做多人公共代理池；
- 破解厂商计费、风控或使用限制；
- 绕过官方封禁/限流；
- 默认开放公网监听；
- 浏览器 Web Dashboard；
- 自动读取所有系统敏感凭据；
- 通过 UI 自动化长期维护 Provider；
- 一开始就做复杂向量数据库长期记忆；
- 一开始就实现几十个 Provider 的“假支持”。

产品定位是**用户本人机器上的本地能力聚合器**。

---

# 4. 成功标准

V0.1 成功必须同时满足：

1. Windows/macOS 均可安装并打开独立 GUI。
2. 首次启动自动扫描本机已安装的目标 Agent。
3. 至少 4 个 P0 Agent Adapter 可真实工作。
4. 至少 2 个 Subscription/Provider Adapter 可真实工作。
5. GUI 中可完成“发现 → 添加 → 检测登录 → 查看模型 → 开启本地 API”。
6. `/v1/models` 正确列出可用模型与虚拟模型。
7. `/v1/chat/completions` 支持非流式与 SSE 流式响应。
8. `/v1/responses` 至少支持核心文本输入、stream 与错误映射。
9. 可创建本地 API Key，Key 默认有权限范围。
10. Provider 失效不会使整个程序崩溃。
11. 可查看模型来源、账号、额度/状态、最近错误。
12. Session 可在支持 resume 的 Agent 上持续。
13. 免费/0-credit/Reserve 状态不可写死；动态检测失败时显示 Unknown。
14. 默认只监听 `127.0.0.1`。
15. Token/Secret 不进入普通日志。
16. 应用退出时正确处理子进程，无孤儿进程。
17. macOS/Windows 打包流程进入 CI。

---

# 5. 参考项目：取长补短，不复制架构

以下项目只作为设计参考，不作为产品依赖或照搬模板。

## 5.1 AgentDock

参考：本机 Agent 自动检测、跨平台桌面形态、Provider/MCP 管理、诊断体验。

- https://github.com/Cailiang/AgentDock

可借鉴：

- 首次扫描与可接入卡片；
- 多种安装来源检测；
- 诊断中心；
- 桌面设置、托盘、自启动。

不可照搬：产品目标不同，LocalMux 的核心是“订阅能力 → API Runtime”。

## 5.2 AgentMux

参考：将真实 CLI Agent 作为一等 Runtime、Session 状态和多 Agent 生命周期管理。

- https://agentmux.ai/

可借鉴：

- Agent process lifecycle；
- session ownership；
- provider isolation；
- structured streaming parser。

## 5.3 BridgeDeck

参考：本地账号/订阅桥接、OpenAI-compatible API、额度/身份诊断。

- https://github.com/papperrollinggery/bridgedeck

可借鉴：

- localhost-first；
- account-scoped routing；
- Usage dashboard；
- OAuth identity 与配置状态诊断。

## 5.4 Submux / Quotamux

参考：订阅态/额度路由，而不是按“本地日志估算”配额。

- https://github.com/adarshba/submux
- https://pypi.org/project/quotamux/

可借鉴：

- 尽量读取 Provider 的真实 quota；
- quota cache；
- 不把本地 token 统计当成官方剩余额度；
- 多套餐池选择策略。

## 5.5 Patchbay / AgentGate 类项目

参考：本地安全边界、凭据保护、per-client 权限、审计日志。

- https://github.com/ultrathinker/patchbay
- https://github.com/dengmengmian/agentgate-ai

## 5.6 专用代理参考

- WorkBuddy: https://github.com/yuan240324/workbuddy-openai-proxy
- Qoder: https://github.com/lininn/qorder-proxy
- Codex: https://github.com/anxkhn/codex-openai-proxy

这些项目用于研究协议转换、错误映射、模型发现、额度信息，但 LocalMux 不直接把多个 proxy 进程堆在一起作为最终架构。

---

# 6. 核心领域模型

必须明确区分以下对象。

## 6.1 Application

本机安装的软件。例如：

```text
Codex Desktop / Codex CLI
Qoder
WorkBuddy / CodeBuddy
Kimi Code
Claude Code
ZCode
TRAE
OpenCode
Gemini CLI
Qwen Code
```

字段：

```text
id
kind
name
version
platform
install_path
executable_path
config_paths[]
detection_source
status
last_scanned_at
```

## 6.2 AgentRuntime

可执行 Agent 能力，例如 Codex app-server、Qoder ACP、Kimi headless。

## 6.3 Provider

提供模型/套餐能力的一方，例如：

```text
OpenAI/ChatGPT Codex Subscription
Qoder Subscription
CodeBuddy Subscription
Kimi Coding Plan
GLM Coding Plan
Volcengine Ark Coding/Agent Plan
Anthropic Subscription
```

## 6.4 Account

某 Provider 下用户的实际登录账户/套餐实例。

## 6.5 Model

账号可调用模型或逻辑档位。

## 6.6 Connection

LocalMux 对某一个 Application/Provider/Account 的已授权接入配置。

## 6.7 Route

从 LocalMux 模型 ID 到实际 Connection + Model 的路由关系。

## 6.8 Session

LocalMux Conversation 与第三方 Agent 原生 session 的映射。

---

# 7. 总体架构

```text
┌────────────────────────────────────────────────────────────┐
│                    LocalMux Desktop                        │
│  Tauri 2 Window / Tray / Notifications / Auto Start       │
├────────────────────────────────────────────────────────────┤
│                    React Desktop UI                        │
│ Dashboard / Discover / Connections / Models / Routing     │
│ API Keys / Sessions / Usage / Logs / Settings             │
├────────────────────────────────────────────────────────────┤
│                    Control Plane (Rust)                    │
│                                                            │
│ Scanner  Catalog  Accounts  Quota  Router  Sessions       │
│ Process Manager  Health  Audit  Settings  Secret Manager  │
├────────────────────────────────────────────────────────────┤
│                    Adapter SDK                             │
│                                                            │
│ AgentAdapter                ProviderAdapter                │
│ Codex                       GLM Coding Plan                │
│ Qoder                       Ark Plan                       │
│ CodeBuddy                   ...                            │
│ Kimi Code                                                   │
│ Claude Code                                                 │
│ Gemini/OpenCode/Qwen                                        │
├────────────────────────────────────────────────────────────┤
│                 Embedded Local Gateway                    │
│ Axum + Tokio                                                │
│                                                            │
│ /v1/models                                                 │
│ /v1/chat/completions                                       │
│ /v1/responses                                              │
│ /api/... native control endpoints                          │
├────────────────────────────────────────────────────────────┤
│ SQLite                OS Credential Store                  │
│ metadata/state        Keychain / Credential Manager        │
└────────────────────────────────────────────────────────────┘
```

---

# 8. 技术栈

## 8.1 Desktop

- Tauri 2.x
- Rust stable（通过 `rust-toolchain.toml` 固定）
- React 19
- TypeScript 5.x
- Vite
- pnpm
- TanStack Query
- Zustand（仅 UI ephemeral state；服务端状态优先 TanStack Query）
- Radix primitives 或等价 accessible primitives

## 8.2 Rust Core

- `tokio`
- `axum`
- `tower` / `tower-http`
- `reqwest`
- `serde` / `serde_json`
- `thiserror`
- `tracing` / `tracing-subscriber`
- `sqlx` + SQLite
- `uuid`
- `chrono`
- `secrecy`
- `keyring` 或平台安全凭据封装
- `sysinfo`（必要范围）
- `which`
- `directories`

不要为了省事引入 Python 作为核心运行依赖。

## 8.3 数据库

SQLite + migration。

不在 V0.1 引入 Postgres/Redis/Qdrant。

---

# 9. 项目目录（从第一次提交就建立）

```text
localmux/
├── README.md
├── README.zh-CN.md
├── LICENSE
├── CHANGELOG.md
├── CONTRIBUTING.md
├── SECURITY.md
├── CODE_OF_CONDUCT.md
├── AGENTS.md
├── rust-toolchain.toml
├── Cargo.toml
├── Cargo.lock
├── pnpm-workspace.yaml
├── package.json
├── pnpm-lock.yaml
│
├── docs/
│   ├── README.md
│   ├── product/
│   │   ├── PRODUCT_SPEC.md
│   │   ├── UX_SPEC.md
│   │   ├── SUPPORTED_APPS.md
│   │   ├── SUPPORTED_PROVIDERS.md
│   │   └── RELEASE_SCOPE.md
│   ├── architecture/
│   │   ├── OVERVIEW.md
│   │   ├── DOMAIN_MODEL.md
│   │   ├── ADAPTER_SYSTEM.md
│   │   ├── DISCOVERY_ENGINE.md
│   │   ├── ROUTING.md
│   │   ├── SESSION_MODEL.md
│   │   ├── QUOTA_MODEL.md
│   │   ├── PROCESS_MODEL.md
│   │   ├── DATABASE.md
│   │   ├── SECURITY.md
│   │   └── OBSERVABILITY.md
│   ├── api/
│   │   ├── OPENAI_COMPAT.md
│   │   ├── RESPONSES_API.md
│   │   ├── NATIVE_API.md
│   │   ├── ERROR_MODEL.md
│   │   └── STREAMING.md
│   ├── adapters/
│   │   ├── README.md
│   │   ├── CODEX.md
│   │   ├── QODER.md
│   │   ├── CODEBUDDY.md
│   │   ├── KIMI.md
│   │   ├── CLAUDE.md
│   │   ├── GLM.md
│   │   ├── ARK.md
│   │   ├── GEMINI.md
│   │   ├── OPENCODE.md
│   │   └── QWEN.md
│   └── development/
│       ├── BUILD.md
│       ├── TESTING.md
│       ├── RELEASE.md
│       ├── DEBUGGING.md
│       ├── ADDING_ADAPTER.md
│       └── CI.md
│
├── apps/
│   └── desktop/
│       ├── package.json
│       ├── src/
│       │   ├── app/
│       │   ├── components/
│       │   ├── features/
│       │   ├── routes/
│       │   ├── hooks/
│       │   ├── lib/
│       │   ├── styles/
│       │   └── main.tsx
│       └── src-tauri/
│           ├── tauri.conf.json
│           ├── capabilities/
│           ├── icons/
│           └── src/
│
├── crates/
│   ├── core/
│   ├── domain/
│   ├── gateway/
│   ├── discovery/
│   ├── router/
│   ├── quota/
│   ├── sessions/
│   ├── process-manager/
│   ├── storage/
│   ├── secrets/
│   ├── security/
│   ├── telemetry/
│   ├── api-types/
│   ├── adapter-sdk/
│   └── adapters/
│       ├── codex/
│       ├── qoder/
│       ├── codebuddy/
│       ├── kimi/
│       ├── claude/
│       ├── glm/
│       ├── ark/
│       ├── gemini/
│       ├── opencode/
│       └── qwen/
│
├── packages/
│   ├── ui/
│   ├── shared-types/
│   └── desktop-api-client/
│
├── migrations/
│   ├── 0001_initial.sql
│   └── ...
│
├── tests/
│   ├── fixtures/
│   │   ├── discovery/
│   │   ├── adapters/
│   │   └── protocol/
│   ├── integration/
│   ├── compatibility/
│   ├── security/
│   └── smoke/
│
├── scripts/
│   ├── verify.sh
│   ├── verify.ps1
│   ├── package-macos.sh
│   └── package-windows.ps1
│
└── .github/
    ├── ISSUE_TEMPLATE/
    ├── PULL_REQUEST_TEMPLATE.md
    └── workflows/
        ├── ci.yml
        ├── desktop-build.yml
        ├── release.yml
        └── security.yml
```

禁止创建平行的 `backend2/`、`new-core/`、`temp-final/` 等结构。

---

# 10. 文档制度

## 10.1 `AGENTS.md`

根目录只做“导航”，控制在 150 行以内。

必须包含：

```text
任务类型 → 必读文档 → 可修改目录 → 必跑测试
```

例如：

```text
Adapter bug:
  Read: docs/architecture/ADAPTER_SYSTEM.md
        docs/adapters/<NAME>.md
  Touch: crates/adapters/<name>/
  Test: cargo test -p localmux-adapter-<name>
        compatibility tests for <name>
```

## 10.2 Adapter 文档模板

每个 Adapter 文档必须有：

```text
Purpose
Supported OS
Detection
Supported versions
Connection/auth strategy
Capabilities
Model discovery
Quota discovery
Free-tier detection
Session handling
Streaming format
Process lifecycle
Error mapping
Security boundary
Known limitations
Fixtures
Compatibility tests
Manual verification checklist
```

## 10.3 ADR

涉及以下变更必须新增 `docs/architecture/adr/ADR-xxxx-*.md`：

- 核心技术栈；
- 数据库结构大改；
- Adapter 契约 breaking change；
- 安全边界变化；
- 默认监听范围变化；
- Provider 私有协议引入。

---

# 11. 本机 Application Discovery Engine

## 11.1 目标

用户安装后点击一次“扫描”，得到：

```text
已安装 / 可连接 / 已连接 / 需要登录 / 版本不支持 / 仅检测不可接入
```

## 11.2 Detection Manifest

每个 Agent 定义静态 Manifest：

```text
id
name
platforms
bundle_ids
executables
known_paths
config_paths
registry_hints
package_managers
minimum_version
capability_hints
```

Manifest 只定义“如何发现”，真正执行逻辑在 Adapter crate。

## 11.3 macOS 扫描

至少检查：

```text
/Applications
~/Applications
PATH
/usr/local/bin
/opt/homebrew/bin
~/.local/bin
~/.cargo/bin
npm/pnpm global bin
常见 nvm/volta/asdf/mise 路径
.app Bundle ID / Info.plist
```

配置路径检查必须只确认“存在/版本/可用性”，不得默认读取 secret 内容。

## 11.4 Windows 扫描

至少检查：

```text
Program Files
Program Files (x86)
%LOCALAPPDATA%
%APPDATA%
%USERPROFILE%\AppData\Local\Programs
PATH
WindowsApps
Registry Uninstall
Registry App Paths
npm/pnpm global
Scoop
WinGet 常见安装目录
Cargo/Pipx 等
```

## 11.5 Scanner 输出

```text
DetectedApplication {
  app_id,
  install_id,
  version,
  executable,
  install_path,
  confidence,
  detection_source,
  candidate_adapter,
  connectability,
  diagnostics[]
}
```

`connectability`：

```text
SUPPORTED
SUPPORTED_AUTH_REQUIRED
SUPPORTED_VERSION_UNKNOWN
DETECTED_ONLY
UNSUPPORTED_VERSION
BROKEN_INSTALL
```

---

# 12. Adapter SDK

## 12.1 Capability Flags

```text
CHAT
RESPONSES
STREAMING
TOOLS
AGENT_RUNTIME
SESSION
SESSION_RESUME
MODEL_DISCOVERY
QUOTA
FREE_TIER
IMAGES
REASONING
MCP
ACP
APP_SERVER
HEADLESS
```

## 12.2 AgentAdapter Trait（概念契约）

```rust
trait AgentAdapter {
    async fn detect(&self, ctx: &DetectContext) -> Result<Vec<DetectedInstallation>>;
    async fn inspect(&self, install: &DetectedInstallation) -> Result<AdapterInspection>;
    async fn connect(&self, request: ConnectRequest) -> Result<Connection>;
    async fn health(&self, connection: &Connection) -> Result<HealthStatus>;

    async fn list_models(&self, connection: &Connection) -> Result<Vec<ModelDescriptor>>;
    async fn get_quota(&self, connection: &Connection) -> Result<Option<QuotaSnapshot>>;

    async fn create_session(&self, req: SessionCreateRequest) -> Result<ProviderSession>;
    async fn resume_session(&self, req: SessionResumeRequest) -> Result<ProviderSession>;
    async fn invoke(&self, req: NormalizedRequest) -> Result<NormalizedResponse>;
    async fn invoke_stream(&self, req: NormalizedRequest) -> Result<NormalizedStream>;
    async fn cancel(&self, run_id: RunId) -> Result<()>;
}
```

## 12.3 ProviderAdapter Trait

用于 GLM Coding Plan / Ark Plan 等非 Agent Runtime 入口：

```rust
trait ProviderAdapter {
    async fn connect(...);
    async fn health(...);
    async fn list_models(...);
    async fn get_quota(...);
    async fn invoke(...);
    async fn invoke_stream(...);
}
```

## 12.4 Adapter 禁止事项

- 直接依赖 GUI DOM；
- 把 token 写入 SQLite；
- 把厂商错误直接透传到用户；
- 在 Adapter 内实现全局路由；
- 在 Adapter 内定义虚拟模型 `free/best`；
- 修改其他 Provider 的配置。

---

# 13. 首批支持矩阵

状态分为：

```text
D = Detect
C = Connect
M = Model Discovery
Q = Quota
S = Session
A = API Invoke
```

## P0

| 对象 | 类型 | D | C | M | Q | S | A | 目标方式 |
|---|---|---:|---:|---:|---:|---:|---:|---|
| Codex | Agent + Subscription | ✓ | ✓ | ✓ | ✓* | ✓ | ✓ | App Server / SDK / CLI headless |
| Qoder | Agent + Subscription | ✓ | ✓ | ✓ | ✓* | ✓ | ✓ | SDK / ACP / Headless |
| WorkBuddy / CodeBuddy | Agent + Subscription | ✓ | ✓ | ✓ | ✓* | ✓* | ✓ | SDK / Headless |
| Kimi Code | Agent + Subscription | ✓ | ✓ | ✓ | ✓* | ✓ | ✓ | Headless / stream-json / ACP |
| GLM Coding Plan | Provider | ✓* | ✓ | ✓ | ✓* | N/A | ✓ | 官方 Coding endpoint / supported config |
| Ark Coding/Agent Plan | Provider | ✓* | ✓ | ✓ | ✓* | N/A | ✓ | 官方 endpoint |

`*` 表示以实际稳定公开能力为准；不存在稳定接口时返回 `Unsupported`，禁止伪造。

## P1

```text
Claude Code
Gemini CLI
OpenCode
Qwen Code
ZCode
TRAE
```

## P2 / client-side integration only

```text
Cursor
Cline
Roo Code
Kilo Code
Continue
```

这类产品很多时候更适合作为 LocalMux API 的消费者，而非上游 Provider。

---

# 14. 模型目录 Model Catalog

统一结构：

```text
ModelDescriptor
├── id                    # localmux canonical id
├── upstream_model_id
├── display_name
├── provider_id
├── account_id
├── connection_id
├── capabilities[]
├── context_window?
├── reasoning?
├── cost_mode
├── quota_multiplier?
├── quota_remaining?
├── reset_at?
├── availability
├── source
└── refreshed_at
```

## 14.1 CostMode

```text
FREE
SUBSCRIPTION
RESERVE
METERED
UNKNOWN
```

## 14.2 免费模型定义

用户所说的 `free` **仅指已连接软件/订阅内部被明确识别为 0-credit / Basic free / Reserve 可用的能力**。

V0.x 默认**不接外部 OpenRouter/Groq/Gemini 免费 API 来填充 `free`**。

免费判定优先级：

1. Provider 明确返回 `credit_multiplier == 0`；
2. Provider 明确标记 Basic/Free tier；
3. Provider 明确返回 Reserve 且当前可用；
4. Adapter 有经测试且文档化的稳定规则；
5. 否则 `UNKNOWN`。

`UNKNOWN` 永远不能进入 `free` 自动池。

## 14.3 动态刷新

```text
应用启动
连接完成
每 5 分钟后台刷新
发生 429/Quota 错误后立即刷新
用户点击刷新
```

刷新使用 per-provider debounce/cache，避免过度调用 quota endpoint。

---

# 15. Routing Engine

## 15.1 路由类型

### Exact Route

```text
codex/<model>
qoder/<model>
workbuddy/<model>
kimi/<model>
glm/<model>
ark/<model>
```

### Virtual Route

```text
free
cheap
balanced
best
```

## 15.2 `free`

只进入：

```text
CostMode == FREE
或明确可消费 RESERVE 且用户允许 reserve_as_free=true
```

没有可用候选时：

```text
HTTP 429 / LOCALMUX_FREE_POOL_EXHAUSTED
```

不得暗中改用收费模型。

## 15.3 `cheap`

优先级：

```text
FREE
→ RESERVE
→ 最低 multiplier 的 SUBSCRIPTION
```

## 15.4 `balanced`

考虑：

```text
health
capabilities
remaining quota
recent latency
session affinity
configured priority
quota multiplier
```

## 15.5 `best`

由用户显式定义优先级或 Model Policy，不允许 LocalMux 自行宣称哪个政治/主观意义上的“最佳”；技术层只根据用户配置的 capability/priority 选择。

## 15.6 Sticky Session

同一 Conversation 默认保持同一 Provider/Model，除非：

- health failure；
- quota exhausted；
- 用户明确切换；
- route policy 允许 failover。

切换 Provider 时必须把事件记录在 run/audit 中。

---

# 16. Session 与“记忆”

## 16.1 V0.1 三层模型

```text
Layer 1: Client message history
Layer 2: LocalMux Conversation / Response mapping
Layer 3: Provider-native Session
```

## 16.2 ProviderSession

```text
provider_sessions
├── id
├── conversation_id
├── provider_id
├── connection_id
├── upstream_session_id
├── state
├── created_at
├── updated_at
└── metadata_json
```

若 Provider 支持 resume，必须优先 resume，而不是每轮新建 Agent。

## 16.3 OpenAI Chat Completions

标准客户端通常会重新发送完整 `messages`。LocalMux 必须正常工作，不要求私有 session header。

可选扩展：

```text
X-LocalMux-Conversation-Id
```

若客户端传入，则启用 provider session affinity。

## 16.4 Responses API

实现：

```text
response_id
previous_response_id
```

在 SQLite 中保存 LocalMux response → provider session 映射。

## 16.5 长期 Memory

V0.1 不实现自动提取偏好/事实的向量记忆。

V0.3 以后再加入：

```text
memory_fact
scope
source
confidence
created_at
last_used_at
```

优先 SQLite FTS，确有需要再评估 embedding/vector store。

---

# 17. Gateway API

默认：

```text
http://127.0.0.1:8317
```

## 17.1 OpenAI-compatible

必须：

```text
GET  /v1/models
POST /v1/chat/completions
POST /v1/responses
```

V0.2：

```text
POST /v1/embeddings        # 仅存在可靠上游时
POST /v1/messages          # Anthropic-compatible，可选
```

## 17.2 Native API

```text
GET    /api/health
GET    /api/apps
POST   /api/apps/scan
GET    /api/connections
POST   /api/connections
DELETE /api/connections/:id
POST   /api/connections/:id/refresh
GET    /api/models
GET    /api/quota
GET    /api/routes
PUT    /api/routes/:id
GET    /api/sessions
GET    /api/runs
POST   /api/runs/:id/cancel
GET    /api/logs
GET    /api/settings
PUT    /api/settings
```

GUI 可以优先通过 Tauri command/internal service 访问核心，但 Native API 仍应保留完整契约，以便测试与未来远程管理。

## 17.3 SSE

Streaming 必须：

- 首 chunk timeout；
- client disconnect cancellation；
- upstream process cancellation；
- clean `[DONE]`；
- provider error → normalized error event。

---

# 18. API Key 系统

首次启用 API 服务生成：

```text
lmx_<random>
```

数据库只保存 hash；明文仅创建时显示一次，可重新生成。

API Key scope：

```text
models:read
chat:invoke
responses:invoke
agent:invoke
sessions:read
usage:read
```

还需要：

```text
allowed_routes[]
allowed_connections[]
execution_profile
rate_limit
```

V0.1 即使是单用户本机应用，也按此 schema 设计，避免以后重构。

---

# 19. Agent 执行权限与安全边界

这是本项目与普通 LLM Proxy 最大的安全差异。

Agent 可能拥有：

```text
读文件
写文件
执行 shell
Git 操作
网络访问
工具调用
```

因此 Connection 必须关联 Execution Profile：

```text
INFERENCE_ONLY
READ_ONLY_AGENT
WORKSPACE_WRITE
FULL_AGENT
```

默认：

```text
OpenAI-compatible API Key → INFERENCE_ONLY / READ_ONLY_AGENT
```

用户必须在 GUI 显式提升权限。

如果 Provider 不支持权限约束，则 GUI 必须清楚提示：

```text
“This runtime controls its own permissions; LocalMux cannot enforce per-tool restrictions.”
```

不得假装具备无法保证的沙箱能力。

---

# 20. Secret 管理

## 20.1 原则

SQLite 不存明文：

```text
access_token
refresh_token
api_key
password
PAT
```

SQLite 只保存：

```text
credential_ref
credential_kind
```

## 20.2 macOS

使用 Keychain。

## 20.3 Windows

优先 Windows Credential Manager / DPAPI 封装。

## 20.4 日志 Redaction

日志层统一 redact：

```text
Authorization
Bearer
api_key
access_token
refresh_token
cookie
email（可配置）
绝对路径（诊断导出时可脱敏）
```

任何 Adapter 不得自行 print 原始响应。

---

# 21. Process Manager

很多 Agent 需要 LocalMux 管理子进程。

统一：

```text
ManagedProcess
├── id
├── adapter
├── pid
├── started_at
├── state
├── stdin
├── stdout_parser
├── stderr_buffer
├── graceful_shutdown
└── kill_policy
```

要求：

- crash isolation；
- timeout；
- cancel；
- process group；
- Windows Job Object 或等价子进程清理；
- macOS process group；
- App 退出不留孤儿进程；
- structured stdout 优先；
- stderr 只作为诊断，不能混入正常模型输出。

---

# 22. GUI 信息架构

## 22.1 First Run Wizard

步骤：

```text
欢迎
→ 自动扫描
→ 显示发现的软件
→ 选择添加
→ 验证连接
→ 创建 LocalMux API Key
→ 显示 Base URL + 测试
```

目标：非技术用户不需要终端操作。

## 22.2 Dashboard

展示：

- Gateway Running/Stopped；
- Base URL；
- 已连接 Provider 数量；
- 当前可用模型；
- Free pool 状态；
- 最近 24h 请求数；
- 当前 Running Agent tasks；
- 健康告警。

## 22.3 Discover（本机应用）

卡片字段：

```text
Logo / Name
Version
Install source
Login status（可知时）
Connectability
Capabilities
[添加] [诊断]
```

## 22.4 Connections

一个账号/订阅一张卡：

```text
Provider
Account alias
Auth state
Plan/Subscription（可知时）
Quota
Models
Health
Last refresh
```

## 22.5 Models

列表支持过滤：

```text
Provider
Free / Subscription / Reserve / Unknown
Tools
Reasoning
Images
Agent
Health
```

## 22.6 Routing

可视化配置：

```text
free
cheap
balanced
best
```

拖动优先级，设置 fallback 与是否允许跨收费档。

## 22.7 API

- Start/Stop Gateway；
- Copy Base URL；
- 创建/删除 Key；
- Key scope；
- Quick test；
- 示例配置片段。

## 22.8 Sessions

- session id；
- provider；
- model；
- created；
- last active；
- status；
- resume capability；
- cancel。

## 22.9 Usage

区别：

```text
Official quota（Provider 上报）
Local request/token statistics（LocalMux 统计）
```

两者绝不能混为一列。

## 22.10 Logs

默认只展示结构化事件，不展示完整 prompt/response。

Debug 模式可由用户显式开启 payload logging，并有醒目隐私提示。

---

# 23. UI 设计规范

目标：系统级工具，不做“网页后台感”。

- 左侧窄导航；
- 内容区高信息密度；
- 支持 light/dark/system；
- 状态颜色必须配文字/图标，不靠颜色单独表达；
- 控件尺寸统一；
- macOS/Windows 分别适配窗口、标题栏与系统行为；
- 不把终端作为主界面；
- 高级诊断可展示 command/path，但默认用户不需要理解。

首版不追求花哨动画。

---

# 24. SQLite Schema（初版）

必须通过 migration 管理。

核心表：

```text
applications
installations
connections
accounts
credential_refs
models
model_capabilities
quota_snapshots
routes
route_targets
api_keys
conversations
provider_sessions
responses
runs
run_events
usage_records
health_checks
settings
audit_logs
```

## 24.1 usage_records

LocalMux 自己统计：

```text
request_id
api_key_id
route
connection_id
model_id
input_tokens?
output_tokens?
cache_tokens?
started_at
duration_ms
status
```

标记：

```text
usage_source = PROVIDER_REPORTED | ESTIMATED | UNKNOWN
```

## 24.2 quota_snapshots

```text
connection_id
scope
remaining
limit
unit
reset_at
source
raw_metadata_json
observed_at
```

不要用 usage_records 推导官方 quota 并伪装成真实剩余量。

---

# 25. 错误模型

统一错误：

```text
LMX_AUTH_REQUIRED
LMX_AUTH_EXPIRED
LMX_PROVIDER_UNAVAILABLE
LMX_MODEL_UNAVAILABLE
LMX_QUOTA_EXHAUSTED
LMX_FREE_POOL_EXHAUSTED
LMX_RATE_LIMITED
LMX_SESSION_NOT_FOUND
LMX_SESSION_RESUME_FAILED
LMX_ADAPTER_UNSUPPORTED_VERSION
LMX_PERMISSION_DENIED
LMX_PROCESS_FAILED
LMX_PROCESS_TIMEOUT
LMX_INVALID_REQUEST
LMX_INTERNAL_ERROR
```

OpenAI-compatible endpoint 转换为标准 HTTP 状态 + OpenAI 风格 error object。

保留内部 `error_code` 在扩展字段中。

---

# 26. Health / Circuit Breaker

每个 Connection 状态：

```text
HEALTHY
DEGRADED
AUTH_REQUIRED
RATE_LIMITED
QUOTA_EXHAUSTED
OFFLINE
UNSUPPORTED
UNKNOWN
```

Router 必须跳过：

```text
AUTH_REQUIRED
OFFLINE
UNSUPPORTED
```

429 后短期 breaker，reset_at 到期后允许 half-open probe。

---

# 27. Adapter 实现优先顺序

## Phase A — Codex

目标：

- Detect install；
- inspect auth；
- list models；
- app-server/SDK/headless invocation；
- stream；
- session create/resume；
- quota/rate-limit 若可稳定获取；
- process lifecycle；
- normalized errors。

禁止将 ChatGPT 登录 token 复制到 LocalMux 自己的数据文件。

## Phase B — Qoder

目标：

- Detect desktop/CLI；
- Headless/ACP/SDK；
- model list；
- session；
- 0-credit/Efficient 等状态若上游明确提供则映射，否则 Unknown；
- 兼容 Global/CN 差异必须由配置或检测区分，不硬猜。

## Phase C — CodeBuddy / WorkBuddy

目标：

- Detect；
- Headless/SDK；
- model list；
- credit multiplier；
- free model dynamic classification；
- streaming；
- session 能力如实上报。

## Phase D — Kimi Code

目标：

- Detect；
- `-p`/headless；
- `stream-json` parser；
- session resume；
- ACP（如稳定）；
- subscription quota（如官方/稳定可获取）。

## Phase E — GLM / Ark

作为 ProviderAdapter，不要求强行绑定某个 GUI Agent。

支持官方 plan endpoint / documented configuration。

---

# 28. Compatibility Test Harness

每个 Adapter 必须有离线 fixture 测试：

```text
version output
model list output
quota output
stream events
session created
session resumed
rate limit error
auth expired error
unknown field forward compatibility
```

测试不得依赖真实账号才能跑完整 CI。

另提供 opt-in live tests：

```text
LOCALMUX_LIVE_CODEX=1
LOCALMUX_LIVE_QODER=1
...
```

Live test 默认不在公共 CI 执行。

---

# 29. API Compatibility Tests

建立 golden tests：

## `/v1/models`

- schema；
- model id uniqueness；
- virtual model visibility。

## `/v1/chat/completions`

- system/user/assistant；
- streaming；
- stop；
- temperature（不支持时明确处理）；
- tools（仅 adapter capability 支持）；
- cancellation；
- malformed requests。

## `/v1/responses`

- text input；
- stream；
- previous_response_id；
- metadata；
- error normalization。

建立一个“小型 OpenAI SDK consumer test”，分别用官方 Python/TS OpenAI SDK 指向 LocalMux，验证无需修改 SDK。

---

# 30. Security Tests

至少包括：

- bind 地址默认 127.0.0.1；
- 无 API Key 请求拒绝；
- revoked key 立即失效；
- secret 不出现在 SQLite；
- secret 不出现在日志；
- malicious path 不可越界；
- process args escaping；
- shell injection；
- malformed SSE；
- oversized body；
- rate limiting；
- LAN mode explicit opt-in。

---

# 31. Packaging

## macOS

支持：

```text
Apple Silicon arm64
Intel x86_64（若依赖允许）
优先 universal 或分别构建
```

Release checklist：

- code signing；
- notarization；
- DMG；
- updater signature；
- clean-machine smoke test。

## Windows

支持：

```text
Windows 10/11 x64
```

优先：

- NSIS installer；
- Authenticode signing（正式发布）；
- clean VM smoke test；
- uninstall 保留/删除用户数据选项。

---

# 32. Auto Update

使用 Tauri updater。

要求：

- 不在 Agent 正在执行时强制重启；
- 下载与安装状态可见；
- 支持“稍后更新”；
- Provider Adapter breaking change 在 release notes 明确说明。

---

# 33. 托盘 / 菜单栏

必须支持：

```text
Gateway: Running / Stopped
Open LocalMux
Start/Stop API
Copy Base URL
Running Tasks: N
Warnings: N
Quit
```

macOS 关闭窗口默认可配置为继续托盘运行。

Windows 同理。

---

# 34. Settings

分类：

```text
General
Gateway
Discovery
Routing
Security
Privacy
Advanced
```

重要设置：

```text
Launch at login
Minimize to tray
API port
Bind address
LAN mode
Request logging
Payload logging
Scan paths
Quota refresh interval
Default route
Reserve counts as free
Crash diagnostics
```

---

# 35. 诊断中心

每个 Provider 提供 `Doctor`：

```text
Application found
Executable valid
Version supported
Auth status
Provider reachable
Model catalog
Quota endpoint
Session support
Gateway invocation
```

生成可复制诊断报告时自动脱敏：

```text
username
home path
email
account id
tokens
keys
```

---

# 36. CI/CD

## ci.yml

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
pnpm lint
pnpm typecheck
pnpm test
compatibility fixtures
```

## desktop-build.yml

Matrix：

```text
macos-latest arm64/x64 strategy
windows-latest x64
```

## release.yml

Tag：

```text
v0.x.y
```

产物：

```text
.dmg
.app archive
.exe installer
checksums
release notes
SBOM（后续可加强）
```

---

# 37. 版本与兼容策略

Adapter 必须支持：

```text
min_supported_version
max_verified_version
```

如果检测到比 `max_verified_version` 更新的版本：

```text
SUPPORTED_VERSION_UNKNOWN
```

允许用户尝试连接，但显示“未验证版本”。

不能因为 vendor minor update 直接让整个 App 启动失败。

---

# 38. Phase 实施路线

## Phase 0 — Repository Foundation

### 目标

只搭骨架，不追功能。

### Tasks

- [ ] 初始化 Git repo
- [ ] Tauri 2 desktop shell
- [ ] Rust workspace
- [ ] pnpm workspace
- [ ] 建立完整目录
- [ ] 建立所有基础文档
- [ ] SQLite migrations
- [ ] tracing
- [ ] OS directories
- [ ] theme/i18n framework
- [ ] CI

### Acceptance

- macOS/Windows CI 可 build 空应用；
- GUI 能打开；
- SQLite 能初始化；
- `AGENTS.md` 可正确导航。

---

## Phase 1 — Core Domain + Gateway

### Tasks

- [ ] domain types
- [ ] adapter-sdk
- [ ] storage repositories
- [ ] secret abstraction
- [ ] embedded Axum server
- [ ] API key auth
- [ ] `/api/health`
- [ ] `/v1/models` mock
- [ ] `/v1/chat/completions` mock echo adapter
- [ ] SSE framework
- [ ] error model

### Acceptance

OpenAI Python/TS SDK 可以对 mock provider 完成 non-stream + stream。

---

## Phase 2 — Discovery Engine + GUI First Run

### Tasks

- [ ] macOS scanner
- [ ] Windows scanner
- [ ] detection manifests
- [ ] Discover page
- [ ] first-run wizard
- [ ] manual rescan
- [ ] diagnostics

### Acceptance

在测试机器上正确发现至少：Codex、Qoder、Kimi/Claude/OpenCode 中两个。

---

## Phase 3 — Codex Adapter

### Tasks

- [ ] detection
- [ ] connection/auth inspection
- [ ] model discovery
- [ ] invocation
- [ ] streaming
- [ ] session/resume
- [ ] cancellation
- [ ] health
- [ ] quota when available
- [ ] fixtures/tests/docs

### Acceptance

第三方 OpenAI SDK → LocalMux → Codex → 返回正常结果。

---

## Phase 4 — Qoder + CodeBuddy Adapters

逐个完成，不并行写半成品。

### Acceptance

至少三个上游可以同时连接，`/v1/models` 合并目录正确，无 ID 冲突。

---

## Phase 5 — Kimi + GLM/Ark Provider

完成 Kimi Agent runtime 与 GLM/Ark subscription provider 两种不同 Adapter 类型，验证架构没有混淆。

---

## Phase 6 — Quota + Free Pool

### Tasks

- [ ] quota schema
- [ ] refresh scheduler
- [ ] cost classification
- [ ] `free`
- [ ] `cheap`
- [ ] fallback
- [ ] circuit breaker
- [ ] GUI routing page

### Acceptance

0-credit 模型动态进入 `free`；状态变化后无需重启自动更新；没有 free 候选时不偷偷使用收费模型。

---

## Phase 7 — Session Continuity

### Tasks

- [ ] conversations
- [ ] provider_sessions
- [ ] Responses `previous_response_id`
- [ ] sticky routing
- [ ] cross-provider fallback metadata
- [ ] session GUI

---

## Phase 8 — Security + Production Packaging

### Tasks

- [ ] Keychain / DPAPI
- [ ] API key scopes
- [ ] execution profiles
- [ ] redaction
- [ ] security tests
- [ ] signed build pipeline scaffolding
- [ ] updater
- [ ] tray/menu
- [ ] auto-start

---

## Phase 9 — P1 Adapters

按顺序：

```text
Claude Code
Gemini CLI
OpenCode
Qwen Code
ZCode/TRAE detection + supported integrations
```

每个 Provider 单独 PR/commit series。

---

# 39. V0.1 Scope 冻结

V0.1 不因为“还可以支持某 Provider”不断扩大。

必须先达到：

```text
稳定桌面安装
稳定扫描
4+真实 Adapter
OpenAI compatibility
Streaming
API Keys
Session
Quota / free classification
Logs / Doctor
macOS + Windows packaging
```

然后封版。

---

# 40. 后续 V0.2

- Anthropic `/v1/messages`；
- multi-account per Provider；
- smarter quota balancing；
- Provider priority profiles；
- LAN mode；
- Tailscale integration（可选）；
- import/export settings；
- more adapters；
- usage charts；
- better OpenAI tool-call conversion。

---

# 41. 后续 V0.3

- MCP endpoint；
- ACP bridge；
- LocalMux native client SDK；
- project/workspace profiles；
- long-term memory（可选）；
- remote node / device pairing；
- per-project routing；
- richer policy engine。

---

# 42. Definition of Done

任何 Feature 只有满足以下条件才算 Done：

```text
[ ] Implementation complete
[ ] Unit tests
[ ] Integration/compatibility tests
[ ] No secret leakage
[ ] Error mapping
[ ] UI loading/error/empty states
[ ] Docs updated
[ ] AGENTS navigation remains accurate
[ ] CHANGELOG Unreleased updated
[ ] macOS build unaffected
[ ] Windows build unaffected
[ ] No new warnings
```

Adapter 额外：

```text
[ ] Detection fixture
[ ] Version fixture
[ ] Auth expired fixture
[ ] Model list fixture
[ ] Stream fixture
[ ] 429/quota fixture
[ ] Manual live verification record
[ ] docs/adapters/<NAME>.md
```

---

# 43. 首次提交必须包含的文件

```text
README.md
README.zh-CN.md
AGENTS.md
CHANGELOG.md
SECURITY.md
CONTRIBUTING.md
Cargo.toml
rust-toolchain.toml
package.json
pnpm-workspace.yaml
apps/desktop/
crates/* skeleton
docs/* skeleton
migrations/0001_initial.sql
.github/workflows/ci.yml
```

即使部分目录只有 README，也必须先建立边界。

---

# 44. Agent 开工顺序（不要跳）

执行 Agent 从空仓库开始时，严格按以下顺序：

```text
1. 创建 repo + docs skeleton
2. 写 AGENTS.md
3. 创建 Tauri shell
4. 创建 Rust workspace/crates
5. 定义 domain model
6. 定义 Adapter SDK
7. SQLite + secrets abstraction
8. Embedded Gateway + Mock Adapter
9. OpenAI compatibility tests
10. Discovery Engine
11. GUI first-run
12. Codex Adapter
13. Qoder Adapter
14. CodeBuddy Adapter
15. Kimi Adapter
16. GLM/Ark ProviderAdapter
17. Quota/Free routing
18. Sessions
19. Security hardening
20. Packaging
21. Release V0.1
```

不要先做漂亮 Dashboard，不要先上 20 个 Adapter。

---

# 45. 第一轮开发完成后的验收场景

## Scenario A — 全新用户

1. 安装 LocalMux；
2. 打开应用；
3. 自动发现 Codex/Qoder/Kimi；
4. 点击 Codex 添加；
5. LocalMux 判断已登录或提示官方登录流程；
6. 模型出现；
7. 开启 Gateway；
8. 创建 Key；
9. 复制 Base URL；
10. OpenAI SDK 调用成功。

## Scenario B — 多 Provider

1. Codex + Qoder + WorkBuddy 已连接；
2. `/v1/models` 同时显示；
3. 调用精确模型；
4. 每个请求路由正确；
5. Provider A 崩溃不影响 B/C。

## Scenario C — free

1. WorkBuddy/Qoder 某档上报 0-credit；
2. 自动进入 `free` pool；
3. `model=free` 成功；
4. 免费档变为非免费；
5. 下一次刷新后自动移出；
6. 无免费候选则明确返回 exhausted。

## Scenario D — Session

1. 调用支持 session 的 Agent；
2. 获取 conversation id；
3. 第二轮继续；
4. Provider 使用 resume；
5. GUI 中可看到同一个 session；
6. App 重启后仍可恢复持久映射。

## Scenario E — Security

1. 未授权客户端访问失败；
2. API Key 被 revoke 后立即失败；
3. SQLite 搜索不到明文 secret；
4. 日志导出不包含 token；
5. LocalMux 默认没有公网监听。

---

# 46. 最终产品体验

用户不需要理解下面这些内部细节：

```text
OAuth token
App server
ACP
CLI headless
stream-json
quota endpoint
child process
SSE conversion
```

用户只看到：

```text
发现 Codex                    [添加]
发现 Qoder                    [添加]
发现 WorkBuddy                [添加]
发现 Kimi Code                [添加]
GLM Coding Plan               [连接]
Ark Coding Plan               [连接]

API 服务                       ● 运行中
http://127.0.0.1:8317/v1      [复制]
lmx_********                   [复制]

模型
Free                           3 可用
Balanced                       6 可用
Best                           已配置
```

这应成为所有实现决策的最终判断标准。

---

# 47. 架构判断原则

遇到未在本文明确的问题时，按以下优先级判断：

```text
安全与可维护性
>
稳定公开接口
>
跨平台一致性
>
用户体验
>
协议兼容性
>
功能数量
>
短期实现速度
```

宁可某 Provider 暂时显示：

```text
“已检测，但当前版本尚无稳定接入方式”
```

也不要依赖一个随第三方应用更新即可失效的脆弱 hack。

---

# 48. 对外一句话定义

**中文**

> 智汇枢（LocalMux）是一款 Windows/macOS 本地 AI Runtime 管理器：自动发现你已经安装和订阅的 Codex、Qoder、WorkBuddy、Kimi、GLM、豆包等 AI 能力，并将它们统一成一个可路由、可观测、可延续会话的 OpenAI 兼容 API。

**English**

> LocalMux is a local-first Windows and macOS AI runtime manager that discovers the agents and AI subscriptions you already use, normalizes their models, sessions and quota into one observable routing layer, and exposes them through a unified OpenAI-compatible API.

---

# 49. 最终交付物

V0.1 Repository 最终至少包含：

```text
完整源代码
macOS build
Windows build
安装器
自动扫描
P0 adapters
OpenAI-compatible gateway
API key management
routing
quota/free pool
session persistence
structured logs
doctor diagnostics
测试体系
CI/CD
完整架构文档
完整 Adapter 文档
用户 README
开发者 CONTRIBUTING
SECURITY
CHANGELOG
```

此时才进入 V0.2 规划。

---

# 50. 执行 Agent 的第一条任务

> 从空仓库创建 LocalMux。只执行 Phase 0（Repository Foundation），不要提前实现真实 Provider。严格按 `PROJECT_PLAN.md` 建立目录、文档、Tauri 2 Desktop shell、Rust/pnpm workspace、SQLite migration 骨架、CI、`AGENTS.md` 与 Mock 页面。完成后运行 macOS/Windows 可执行的静态/CI 验证，输出目录树、测试结果、已完成项和 Phase 1 的明确入口。禁止为了“看起来完成更多”提前接入 Codex/Qoder 等 Provider。
