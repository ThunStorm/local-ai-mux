# LocalMux / 智汇枢

LocalMux 是 Tauri 2 桌面应用：扫描本机 AI 软件，用户显式连接有官方接入能力的运行时后，通过 `127.0.0.1:8317` 提供 OpenAI 兼容文本 API。发现或登记应用不代表订阅已连接、已登录或已通过真实请求验收。本机 LocalMux Key 只用于网关认证；供应商 API Key 是独立的用户授权连接。[当前支持矩阵](docs/product/SUPPORTED_APPS.md)和[交接记录](docs/HANDOFF.md)分别列出能力边界与实测状态。

准备 Rust 1.98.1、Node.js 22+、pnpm 11.19.0 及 [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/) 后运行 `pnpm install`、`pnpm dev`；运行 `bash scripts/verify.sh` 验证。增量工作见[产品对齐计划](docs/LOCALMUX_PRODUCT_ALIGNMENT_PLAN_2026-09-28.md)。
