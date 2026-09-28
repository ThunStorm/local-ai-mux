# 架构总览

桌面由 Tauri 2 承载 React UI 与 Rust 控制层。领域类型、Adapter SDK、SQLite、OS 凭据库和嵌入式 Axum Gateway 已建立。网关仅绑定 `127.0.0.1:8317`，由桌面命令显式启停；生产桌面启动时没有 Mock 模型，只有用户显式保存的连接可注册。`mock/echo` 只用于离线测试与兼容性服务。

扫描由 `crates/discovery` 只读执行；桌面控制层用静态注册表把官方 CLI 与 Adapter 关联，Gateway 提供模型、Chat 和 Responses 文本接口。额度独立保留来源及观测时间，Codex 主/次窗口在 `quota_buckets` 中分开存储，旧路由继续读取单快照。当前增量架构和未完成的 connection instance 边界见[产品对齐计划](../LOCALMUX_PRODUCT_ALIGNMENT_PLAN_2026-09-28.md)。
