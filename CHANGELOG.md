# Changelog

## Unreleased

## 0.3.1 — 2026-11-24

- Responses 在获取 Agent 执行权后复核父响应，成功映射与调用终态同事务提交。流式启动超时与 Chat 使用相同的连接冷却，拒绝切换型号的审计事件过滤无效型号。正文录制对无匹配的脱敏规则复用文本，保留逐块持久化和跨块脱敏。

- macOS 本机部署在替换现有应用前校验 Mach-O 支持当前机器的原生架构。

## 0.3.0 — 2026-11-24

- 新增架构导航、请求生命周期和数据治理说明，统一项目和 macOS 预发布入口。
- 新增由 GitHub Actions 构建 Apple Silicon 应用的预发布工作流。
- 清理过时的发布占位工作流、空目录标记和本机验收产物。保留便携设计原型与离线测试 fixture。

- Qoder CLI 保留有界 stderr 供首包超时归因，并正确处理空的 JSON error 字段。

- Qoder CLI 明确报告首包超时时，透传超时层和时限。连接冷却返回剩余等待、冷却原因及本次未发出标记。

- 新增偏好设置“数据与日志”：独立业务数据路径、下一次启动一致快照迁移与失败恢复、固定本机Key/单实例保护、分项占用与小时净增采样；脱敏文件日志按5MiB/UTC日gzip轮转，可配置保留天数和总预算，不自动清理会话正文或人工备份。

- 修正 Windows 下同步 SQLite 迁移快照所需的文件句柄权限。

- 修复Windows存储测试临时数据库清理的偶发共享冲突：独立临时目录、重开前释放旧池、关闭后仅对OS32/33有限重试；新增真实Windows删除锁回归。

## 0.2.0 — 2026-10-26

- 将CLI空闲重置回归改为跨平台虚拟时钟测试，消除CI高负载Shell调度导致的误超时；复用CLI/Codex有界事件读取。

- 修复Unix可靠性测试专用导入在Windows上的未使用导入错误，保留-D warnings门禁。

- 对齐SDK/打包工作流所需Rust组件，修复Ubuntu首次Cargo补装rustfmt的cargo-fmt冲突；固定Python SDK封版基线。

- 修复限时设置返回未知时误显示旧值、初始化/重读竞态；HTTP非流响应与目录有界读取，Chat/Responses非流正文统一8MiB上限，超限返回LMX_OUTPUT_TOO_LARGE。
- 产品使用说明改为中文单一来源，补充隐性问题审查和验收边界。

- 将CLI空闲配置扩大至30–1200秒，保留180秒默认与既有配置；Gateway入站总期同步1200秒，health附LocalAiMux服务名/版本，具体超时层继续记录。
- 产品名称统一LocalAiMux，Cargo workspace、前端与Tauri版本统一0.2.0；保留既有数据目录、bundle identifier与协议身份。
- 收敛重复限值和版本元数据，加入封版版本/品牌/文档链接检查，更新根说明/支持矩阵/API/构建/安全，归档历史交接并集中TODO。
- 下列条目一并纳入本次封版；其中提到的旧计时值与界面尺寸是各次变更当时的演进，当前参数以v0.2.0契约为准。

- 压缩会话筛选与导出工具区，默认窗口首屏可见记录；完整说明按需展开。

- 会话记录新增开始时间段与已录正文筛选；全部匹配结果可通过原生保存窗口导出JSON/Markdown/XLSX/CSV/TXT/HTML，保留录制状态与未知值，长正文分段、文件原子保存。

- CLI 空闲限时可在 API 页配置并持久保存（30–600秒，默认180），新请求使用入站快照；Chat/Responses 与调用详情记录实际超时层和限值，未知超时不再误归总期。

- Fix Routing official quota to share Apps & connections' multi-window buckets, polling, freshness and rendering; retain unknown values and independent reset times, and hide disconnected entries. Correct the CodeBuddy guide to avoid implying WorkBuddy quota sharing.
- Recheck public Qoder SDK account/request/session Credits and WorkBuddy personal-credit OAuth API; document integration, desktop authorization and session-attribution limits without adding provider calls or claiming live support.

- Show provider-reported input/cache counts and a valid cached share in call details; persist optional bounded diagnostic counts without migrating or backfilling history, preserve unknown/zero semantics and honor store:false.

- Project validated per-request cached input counts into standard Chat/Responses usage details while retaining the localmux extension, unknown values and unchanged totals; preserve the last trustworthy usage snapshot across cumulative completion events.
- Declare endpoint-specific native session resume and unsupported explicit prompt-cache control in the existing v2 capability details, with current public-interface evidence, expiry and strict-schema regression coverage; retain stateless Chat and existing Responses isolation.

- Hide the running macOS Dock icon when closing to the tray; restore the regular application policy, window and focus when reopening from the tray or application entry.

- Automatically derive Codex instruction and effort support from the installed CLI public schema on catalog refresh and invocation, replacing version allowlists; bound schema export/reads, intersect model efforts, omit unverified optional fields for user-only defaults and fail closed for explicit instructions.

- Verify and deploy the final macOS reliability tree after the parallel tray package: migrate to schema 13 while retaining the stable key/scopes, connections, recording setting and existing history; preserve unverified upstream/Windows/ARM64 boundaries.

- Gate reliability with isolated official Python/TypeScript SDK servers (25 contract cases each), 400/429/503 and cancellation/truncation coverage, direct loopback transport, bounded diagnostics/storage measurements and the existing cross-platform CI.

- Preserve catalog observation timestamps at refresh and retain the last verified request-usage snapshot when a final event reports untrusted cumulative usage; keep persisted and wire metadata consistent.

- Indicate a running API with a red sun on the white macOS tray silhouette; atomically restore the system template when stopped or exited, refreshing on state changes without repainting unchanged icons.

- Enforce stream deadlines independently of downstream polling with one buffered frame and an abort-on-drop producer; release lanes and owned work even when an SSE body remains unconsumed. Keep 15-second heartbeats and existing execution limits.

- Bound CLI catalog/JSONL reads before allocation grows past 1 MiB and share isolated owned process-group cleanup across Codex/Qoder/CodeBuddy; test cancellation, malformed output, spawn failure and unrelated-process preservation without upstream calls.

- Validate per-request metadata provenance, reject cumulative/unverified usage and bounded-model self reports, and cap the fixed localmux extension at 4096 bytes; correlate response/stream errors with X-Request-ID while preserving standard response IDs.

- Add migration 0013 with optional bounded call diagnostics, independent queue/spacing/adapter timings and request correlation; retain opt-in body recording, store:false and legacy nulls. Extend existing history, local connection state and capability evidence panels.

- Add compatible v2 capability/catalog freshness and evidence expiry; downgrade stale effective claims to UNKNOWN without hiding models or upgrading fixture evidence to live verification.

- Bound Gateway requests from HTTP admission through auth, route, Agent queue/spacing, execution and response consumption with one 600-second monotonic deadline; keep existing limits and add safe X-Request-ID correlation.

- Replace the LocalAiMux app, sidebar and favicon icons with user-selected wave/red-sun option 04; regenerate PNG/ICNS/ICO from the preserved original and archive the previous SVG. Use a separate transparent wave/sun template silhouette in the macOS menu bar.

- Add a persistent opt-in dialogue-recording switch with full submitted messages/input and received answers, including partial streams; stop active capture atomically when disabled and honor store:false.
- Add migration 0012, credential-redacted content storage, on-demand safe dialogue viewing and cascade cleanup; preserve existing metadata, sessions and historical records without backfilling text.

- Polish the call-history toolbar with consistent 36px controls, aligned field labels, grouped responsive actions and restrained cleanup emphasis; move legacy session refresh beside its own session list.

- Expand default call metadata history to Chat, Responses, desktop playground and explicit model tests, including failures, timeouts, cancellation, truncation, filtering and interruption; keep native resume mappings independent and honor standard `store:false`.
- Add migration 0011, restart recovery, stable paginated filters and Responses call grouping; offer details and confirmed cleanup while preserving active calls and existing sessions. Keep metadata separate from opt-in dialogue content; never store credentials, request parameters, client metadata or raw provider errors in the audit tables.

- Separate 600-second Gateway execution deadlines from 180-second stream/CLI idle timeouts; bound Responses non-streaming/resume and continuous streams as well as Chat.
- Serialize Agent connections across models, keys and Chat/Responses/model verification with a 5-second start interval and a 30-second queue timeout; release on failures/cancellation and expose read-only execution limits in health.
- Preserve CLI result quota/auth categories without retaining provider text and distinguish idle timeout from generic process failure. Verify with virtual clocks and local fixtures only.

- Retain the development default API key and explicit Agent permission across restarts/deployments; migrate the old memory key once, then fail explicitly on missing/corrupt/revoked credentials instead of silently rotating.
- Persist only the local API credential in an owner-only Unix file (0700 directory / 0600 file); keep SQLite hashes/permissions and third-party credential behavior unchanged. Windows OS-store integration remains pending native acceptance.
- Separate permission saving from explicit key replacement, preserve key identity and route/connection/rate limits, and document the formal-product access-control TODO list.

- Implement layered capability provenance with schema-versioned, Key-filtered catalogs; preserve all 55 audited IDs and Codex per-model modalities/effort enums without issuing inference requests.
- Support leading system instructions through public CLI/App Server controls, separate Codex developer instructions and declared reasoning effort, explicit standard output-limit conflicts and response-format diagnostics. Keep unverified token limits and JSON guarantees unavailable.
- Carry typed stream/response metadata, reported usage snapshots and actual upstream identity; use null for unknown usage/finish rather than zero/stop. Preserve truncation/refusal/cancellation across Chat, Responses and playground history; isolate runtime configuration and keep explicit Agent authorization.
- Verify generic negotiation with both official OpenAI SDKs, public-protocol fixtures and an opt-in metadata-only installed catalog check. Keep client business adaptation, new history/vision/tools and live-provider acceptance separate.

- Respect existing macOS fixed HTTP(S) proxies for Codex App Server children when no explicit proxy environment is set, while bypassing loopback and preserving user settings.
- Handle Codex structured turn errors promptly, allow slow first tokens within a 120-second stream idle budget with SSE heartbeats, and distinguish provider cooldown (503) from missing models (404) in Chat, Responses and the desktop playground.

- Fix Codex thread start/resume sandbox enum spelling (`read-only` versus turn policy `readOnly`), with a strict protocol regression fixture.
- Allow an unset CodeBuddy model override to use explicitly reported official CLI help model IDs from an isolated directory; retain UNKNOWN costs and user-triggered connection, and document the separate WorkBuddy integration limits.

- Replace the LocalAiMux app mark with a deterministic SVG AI/multiplexer design; restore original installed application/CLI icons and normalize only their presentation frames. Generated vendor illustrations are superseded; absent source assets use an explicit neutral fallback.

- Implement the Liquid Glass desktop redesign with public macOS AppKit backing, accessible opaque fallbacks, refined sidebar/list/composer spacing, an AI-and-multiplexer app icon, and seven original illustrated Agent identities.
- Add an in-memory chat playground using the current authorized model directory and fixed loopback Gateway, capability-gated parameter overrides, isolated model contexts, streaming, request-scoped cancellation, and explicit partial/error states without storing message bodies or exposing keys to JavaScript.
- Preserve supported Chat parameter overrides end to end, reject unknown/out-of-range capabilities, expose an authenticated chat capability directory, and prevent window/app exit while inference is active.

- Document the proposed Apple HIG aligned Liquid Glass redesign and an in-app chat playground, with capability-gated parameters, explicit single/multi-turn boundaries, staged acceptance criteria, and a design-only interactive prototype. Production UI and real model calls are not implemented by this design deliverable.

- Show only applications with an implemented CLI or official loopback bridge; let users hide and restore individual app rows without deleting connections.
- Check official Codex/Qoder/Qoder CN login state without exposing account identifiers; show sign-in only when required, confirm it automatically with bounded polling, and provide separate connection and real-model verification actions.
- Detect the separately installed Qoder CN CLI and route it through an independent `qoder-cn/` Adapter identity.
- Parse the real Qoder CLI `MODEL` table after login; show Qoder and Qoder CN separately, and split WorkBuddy from CodeBuddy CN and standalone CodeBuddy Code CLI without reassigning a login or model catalog.
- List reported model IDs in each application's diagnostics, while keeping the API selector limited to models authorized for the current Key.
- Migrate existing desktop inventory identities in `0010` while preserving connection records; stop treating WorkBuddy's incomplete internal script or CodeBuddy CN's editor launcher as a model CLI.
- Rename the visible desktop app and installed bundle to LocalAiMux while continuing to use the existing LocalAiMux data directory.
- Add user-triggered official sign-in entry points for Codex/Qoder and desktop login entry points for WorkBuddy/MiMo; keep errors on the page where they occurred.
- Map unauthenticated Qoder catalog requests to login guidance; identify WorkBuddy's incomplete bundled CLI instead of offering a connection that cannot use the documented commands.
- Detect the installed MiMo Code CLI separately from its desktop app; add saved-connection model refresh and a direct read-only Agent Key authorization action.
- Connect MiMo Code through its documented loopback capability API on an explicit user action, restrict its short-lived token to reported MiMo models, and revoke it on disconnect; make HTTP Provider catalogs routable after validation.
- Keep MiMo API secrets process-only; the earlier memory-only default-key policy is superseded by the stable development-key behavior above.

### Product alignment

- Define a macOS visual system and rebuild the desktop shell with fixed sidebar navigation, compact searchable application rows, an API configuration flow, explicit light/dark tokens, and a resizable native window.
- Add a factual connection overview and next action, reset scroll position on navigation, and keep the appearance menu inside a narrow window.

- Preserve ordered Chat roles/history for HTTP providers; Agent bridges now reject unsupported multi-message requests with an explicit 400 instead of silently dropping context. Reject unsupported Chat parameters and multi-item Responses input.
- Record the desktop-subscription-first scope and distinguish documented, mock-tested, and live-verified Adapter capabilities.
- Use a static desktop Adapter registry, keep the production Gateway free of Mock models, and add a default local API key with explicit rotation and a one-page model/config copy flow. Move legacy routing and sessions behind Advanced.
- Group macOS app bundles and CLI candidates into one application card, retaining runtime paths and diagnostics instead of displaying them as separate products.
- Offer an explicitly triggered short real-model test; only a successful upstream response marks that model verified for the current UI session.
- Persist separate Codex primary/secondary usage windows in a new quota-bucket migration, keep the legacy single snapshot for routing, and show stale/unknown state on app cards.
- Keep loopback API and local HTTP Provider requests out of system proxies, including their bearer keys.

### Added after Phase 8

- Detect WorkBuddy's bundled official CodeBuddy CLI, expose an explicit Xiaomi MiMo API Key connection, read the Codex App Server quota window, and show quota status for every connected service even when the source reports no balance.
- Detect Xiaomi MiMo Desktop and Ollama alongside existing desktop Agent manifests; explicitly add/remove detected desktop apps in a separate SQLite inventory without registering a Provider connection.

### Changed

- Discover Qoder CLI from the official macOS installer entry directory even when the desktop process has no shell PATH.
- Gate Unix-only Gateway security-test helpers on Unix and use the platform candidate filename in the Discovery fixture so Windows CI can run both checks.
- Added task-routed change workflow and a macOS build/install/restart script with build provenance.

### Added in Phase 8

- Phase 8: OS credential-store implementation, API Key route/connection/profile scopes and per-key rate limit; default keys cannot invoke Agents.
- Security regression coverage for scope, rate, size and revocation; centralized log redaction and restrictive production CSP.
- Responses sessions are bound to the creating API key; legacy sessions with unknown ownership remain visible but cannot be resumed through the API.
- Desktop key management, tray actions/status, opt-in login startup and close-to-tray settings, and signed-update candidate pipeline scaffold.
- Updater remains disabled until a trusted public key and HTTPS endpoint are configured; no OS-signed release has been issued.

### Added in Phase 0

- Phase 0 scaffold from `main`: Tauri 2 + React/TypeScript desktop window, theme and Chinese/English copy.
- Rust and pnpm workspaces, OS data directory, SQLite migration bootstrap, tracing initialization.
- Scoped documentation, CI for macOS/Windows, and local verification scripts.

### Added in Phase 7

- Text Responses API with SDK-compatible non-stream/stream events and latest-response chaining.
- Transactional Conversation/ProviderSession/Response ID storage without prompt or answer bodies.
- Sticky native session resume, rejected route-change audit events, and read-only Sessions desktop view.
- Official Python/TypeScript SDK Responses smoke checks passed against Mock Gateway; real Provider resume remains pending live acceptance.

### Added in Phase 6

- Quota source/snapshot model, per-connection refresh debounce and SQLite quota/route migrations.
- Dynamic free/cheap/balanced/best routing, reserve availability gate, configured paid route opt-in and circuit breaker.
- Desktop Routing page with explicit policy controls and official quota unknown state.
- Synthetic free classification transition test proves no metered fallback.

### Added in Phase 5 (isolated, not production eligible)

- Kimi Agent boundary reports Unsupported until inference-only permissions can be verified.
- GLM and Ark Coding Plan HTTP Provider components use exact dedicated endpoints, in-memory secrets, no redirects, and local mock SSE tests.
- No real Provider key or billing validation was performed; components are not registered in the desktop Gateway.

### Added in Phase 4 (code, live acceptance pending)

- Qoder headless and CodeBuddy CLI adapters with conservative dynamic catalogs, isolated inference-only execution and session IDs.
- Shared JSONL CLI process helper and three-adapter offline merge/routing test.
- Qoder/CodeBuddy real CLI installation, login and live SDK acceptance remain pending.

### Added in Phase 3 (code, live acceptance pending)

- Codex App Server adapter with explicit desktop connection, dynamic model catalog, read-only turns, session resume and incremental SSE.
- Persisted connection path, offline protocol fixture and path validation before launch.
- Real Codex account/model/inference validation remains pending user connection and human review.

### Added in Phase 2

- Read-only macOS/Windows application discovery with static manifests, registration-path checks and explicit detection-only status.
- Desktop first-run scan, Discover diagnostics and rescan; API controls exposed in the desktop UI.
- Cross-target Windows discovery check and macOS false-positive regression test.

### Added in Phase 1

- Domain types, Adapter SDK and Mock echo model; SQLite API Key hash repository and secret-store abstraction.
- Loopback Axum Gateway with scoped API Key auth, models, text chat, SSE and normalized errors.
- Tauri commands for Gateway start/stop/status and Key creation/revocation.
- Official Python and TypeScript OpenAI SDK compatibility smoke checks passed for Mock HTTP server.
