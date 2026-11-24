#!/usr/bin/env bash
# Independent synthetic servers for each official SDK; no installed app or provider credentials.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
export LOCALMUX_TEST_API_KEY="${LOCALMUX_TEST_API_KEY:-lmx_test_offline}"
export LOCALMUX_TEST_PORT="${LOCALMUX_TEST_PORT:-18317}"
[[ "$LOCALMUX_TEST_API_KEY" == lmx_test_* && "$LOCALMUX_TEST_PORT" =~ ^[0-9]+$ && "$LOCALMUX_TEST_PORT" -ge 1024 && "$LOCALMUX_TEST_PORT" -ne 8317 && "$LOCALMUX_TEST_PORT" -le 65535 ]] || { echo "Synthetic key and isolated test port required" >&2; exit 1; }
export LOCALMUX_TEST_BASE_URL="http://127.0.0.1:$LOCALMUX_TEST_PORT/v1"
export NO_PROXY="127.0.0.1,localhost"
export no_proxy="$NO_PROXY"
server=""
cleanup() { if [[ -n "$server" ]]; then kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; server=""; fi; }
trap cleanup EXIT
cargo build -p localmux-gateway --features compat-harness --bin localmux-compat-server
for sdk in python typescript; do
  target/debug/localmux-compat-server &
  server=$!
  ready=false
  for _ in {1..100}; do
    kill -0 "$server" 2>/dev/null || { echo "Synthetic server exited" >&2; exit 1; }
    if curl --noproxy '*' --silent --fail "http://127.0.0.1:$LOCALMUX_TEST_PORT/api/health" > /dev/null; then ready=true; break; fi
    sleep .1
  done
  [[ "$ready" == true ]] || { echo "Synthetic server unavailable" >&2; exit 1; }
  if [[ "$sdk" == python ]]; then "${LOCALMUX_TEST_PYTHON:-python3}" tests/compatibility/openai_python_smoke.py; else node tests/compatibility/openai_ts_smoke.mjs; fi
  cleanup
 done
