# LocalMux / 智汇枢

A local-first desktop AI runtime manager. This branch contains only Phase 0 repository foundation from `main`; no real provider is connected.

## Develop

Prerequisites: Rust 1.98.1, Node.js 22+, pnpm 11.19.0, and [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
pnpm install
pnpm dev
```

Run `bash scripts/verify.sh` for the local checks. Current desktop window is a scaffold. Phase 1 will add the mock gateway; see [handoff](docs/HANDOFF.md).
