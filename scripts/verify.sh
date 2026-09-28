#!/usr/bin/env bash
set -euo pipefail
cargo fmt --check
cargo test --workspace
pnpm typecheck
pnpm test
pnpm build
