# 架构总览

桌面由 Tauri 2 承载 React UI 与 Rust 控制层。Phase 1 已建立独立的领域类型、Adapter SDK、SQLite Key 仓储和嵌入式 Axum Gateway。网关仅绑定 `127.0.0.1:8317`，由桌面命令显式启停；初始只注册 `mock/echo`，未连接第三方账号。

后续按计划在 `crates/discovery`、`crates/adapters/*`、`crates/router`、`crates/quota`、`crates/sessions` 分别实现，避免将 Provider 判断散落在 GUI 或 Gateway。
