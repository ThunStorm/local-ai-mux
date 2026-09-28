$ErrorActionPreference = 'Stop'
cargo fmt --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test --workspace
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
pnpm typecheck
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
pnpm test
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
pnpm build
exit $LASTEXITCODE
