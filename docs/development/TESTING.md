# 验证

基础检查运行 `bash scripts/verify.sh`（Windows 使用 `scripts/verify.ps1`）。Rust 测试覆盖 SQLite migration、Key 撤销、鉴权、模型列表、聊天、SSE、数据库无明文 Key，以及发现引擎的重复路径和 Bundle ID 误报。Phase 2 的 Windows 扫描分支可用 `cargo check -p localmux-discovery --target x86_64-pc-windows-msvc` 静态验证，仍需 Windows CI 实机运行。

官方 SDK 复验：设置合成测试 Key `LOCALMUX_TEST_API_KEY=lmx_test_example` 与 `LOCALMUX_TEST_PORT=18317`，运行 `cargo run -p localmux-gateway --features compat-harness --bin localmux-compat-server`。另开终端设置 `LOCALMUX_TEST_BASE_URL=http://127.0.0.1:18317/v1`，安装 Python `openai` 后运行 `tests/compatibility/openai_python_smoke.py`，并运行 `node tests/compatibility/openai_ts_smoke.mjs`。测试服务只接受 `lmx_test_` 前缀 Key 并绑定 loopback。
