# Changelog

## Unreleased

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
