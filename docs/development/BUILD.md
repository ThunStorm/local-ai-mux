# Build

Install Rust 1.98.1, Node.js 22+, pnpm 11.19.0 and Tauri 2 system prerequisites. Run `pnpm install`, `pnpm dev`, `bash scripts/verify.sh`, or `pnpm tauri build`. The desktop starts its loopback API only when requested; `bash scripts/deploy-local-macos.sh` builds the current working tree, installs to `~/Applications/LocalMux.app`, restarts it, and records provenance in `dist/localmux/build-info.txt`. Check active tasks before deployment. Windows native installation is outside the current Mac-focused acceptance scope.
