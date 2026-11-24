#!/usr/bin/env bash
# Build this checkout's macOS app, install it for the current user, and restart it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
for candidate in "$HOME"/.nvm/versions/node/v*/bin/node; do
  [[ -x "$candidate" && -x "$(dirname "$candidate")/pnpm" ]] || continue
  if [[ "$("$candidate" -p 'process.versions.node.split(".")[0]')" -ge 22 ]]; then
    export PATH="$(dirname "$candidate"):$PATH"
  fi
done
node -e 'if (Number(process.versions.node.split(".")[0]) < 22) process.exit(1)'
export PATH="$HOME/.cargo/bin:$PATH"
bash scripts/package-macos.sh

SRC="$ROOT/target/release/bundle/macos/LocalAiMux.app"
DEST="$HOME/Applications/LocalAiMux.app"
LEGACY="$HOME/Applications/LocalMux.app"
test -d "$SRC" || { echo "missing app: $SRC" >&2; exit 1; }
APP_PROCESS="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$SRC/Contents/Info.plist")"
APP_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$SRC/Contents/Info.plist")"
NATIVE_ARCH="$(uname -m)"
if [[ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" == 1 ]]; then
  NATIVE_ARCH=arm64
fi
if ! lipo "$SRC/Contents/MacOS/$APP_PROCESS" -verify_arch "$NATIVE_ARCH"; then
  echo "LocalAiMux build does not support native architecture $NATIVE_ARCH; installation cancelled" >&2
  exit 1
fi
mkdir -p "$HOME/Applications" "$ROOT/dist/localmux"
STAGE="$(mktemp -d "$HOME/Applications/.localmux-deploy.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT
ditto "$SRC" "$STAGE/LocalAiMux.app"

if pgrep -x "$APP_PROCESS" >/dev/null; then
  osascript -e "tell application id \"$APP_ID\" to quit"
  for _ in {1..20}; do
    pgrep -x "$APP_PROCESS" >/dev/null || break
    sleep 1
  done
  if pgrep -x "$APP_PROCESS" >/dev/null; then
    echo "LocalAiMux is still running; installation cancelled" >&2
    exit 1
  fi
fi

BACKUP="$STAGE/previous.app"
if [[ -d "$LEGACY" ]]; then mv "$LEGACY" "$STAGE/legacy.app"; fi
if [[ -d "$DEST" ]]; then mv "$DEST" "$BACKUP"; fi
if ! mv "$STAGE/LocalAiMux.app" "$DEST"; then
  if [[ -d "$BACKUP" ]]; then mv "$BACKUP" "$DEST"; fi
  if [[ -d "$STAGE/legacy.app" ]]; then mv "$STAGE/legacy.app" "$LEGACY"; fi
  exit 1
fi
if ! open "$DEST"; then
  rm -rf "$DEST"
  if [[ -d "$BACKUP" ]]; then mv "$BACKUP" "$DEST"; fi
  if [[ -d "$STAGE/legacy.app" ]]; then mv "$STAGE/legacy.app" "$LEGACY"; fi
  exit 1
fi
sleep 2
if ! pgrep -x "$APP_PROCESS" >/dev/null; then
  rm -rf "$DEST"
  if [[ -d "$BACKUP" ]]; then mv "$BACKUP" "$DEST"; fi
  if [[ -d "$STAGE/legacy.app" ]]; then mv "$STAGE/legacy.app" "$LEGACY"; fi
  echo "LocalAiMux did not stay running" >&2
  exit 1
fi
ditto "$DEST" "$ROOT/dist/localmux/LocalAiMux.app"
printf 'source_root=%s\nbranch=%s\ncommit=%s\nworking_tree=%s\nbinary_sha256=%s\n' "$ROOT" "$(git branch --show-current)" "$(git rev-parse --short HEAD)" "$(if [[ -n "$(git status --porcelain)" ]]; then echo modified; else echo clean; fi)" "$(shasum -a 256 "$DEST/Contents/MacOS/$APP_PROCESS" | awk '{print $1}')" > "$ROOT/dist/localmux/build-info.txt"
echo "Installed and restarted: $DEST ($(git branch --show-current) $(git rev-parse --short HEAD))"
