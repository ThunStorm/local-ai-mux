# LocalMux / 智汇枢

LocalMux is a Tauri 2 desktop app that discovers installed AI applications and, after you explicitly connect a supported official runtime, exposes reported models through a loopback OpenAI-compatible text API. Detection alone never grants access or proves that a subscription can make API calls. [中文说明](README.zh-CN.md) · [Current support matrix](docs/product/SUPPORTED_APPS.md) · [Verified handoff](docs/HANDOFF.md).

The current API is bound to `127.0.0.1:8317`. Its LocalMux API key authenticates the local gateway; any provider API key is a separate, user-authorized connection. Chat role/history support depends on the adapter, and unsupported inputs return an error. Live subscription acceptance remains pending user-initiated connection.

## Develop

Prerequisites: Rust 1.98.1, Node.js 22+, pnpm 11.19.0, and [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
pnpm install
pnpm dev
```

Run `bash scripts/verify.sh` for the local checks. The [product alignment plan](docs/LOCALMUX_PRODUCT_ALIGNMENT_PLAN_2026-09-28.md) describes the incremental work; [AGENTS.md](AGENTS.md) gives task-specific checks.

The [capability negotiation design](docs/architecture/CAPABILITY_NEGOTIATION_DESIGN.md) is a review draft for generic API/Adapter improvements; the [2026-10-06 model audit](docs/architecture/CAPABILITY_AUDIT_2026-10-06.md) separates current Gateway limits, public runtime features, and model metadata for all 55 connected model IDs. These documents do not claim that the proposed changes are implemented or live-verified.
