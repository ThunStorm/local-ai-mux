# Changelog

## Unreleased

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
- Rename the visible desktop app and installed bundle to LocalAiMux while continuing to use the existing LocalMux data directory.
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
