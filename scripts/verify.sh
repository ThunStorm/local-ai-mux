#!/usr/bin/env bash
set -euo pipefail
node scripts/check-release.mjs
cargo fmt --check
cargo test --workspace
pnpm typecheck
pnpm test
pnpm build
