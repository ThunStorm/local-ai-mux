# LocalAiMux 架构总览

桌面由 Tauri 2 承载 React UI 与 Rust 控制层。领域类型、Adapter SDK、SQLite、凭据存储抽象和嵌入式 Axum Gateway 已建立。网关仅绑定 `127.0.0.1:8317`，由桌面命令显式启停；生产桌面启动时没有 Mock 模型，只有用户显式保存的连接可注册。`mock/echo` 只用于离线测试与兼容性服务。

扫描由 `crates/discovery` 只读执行；桌面控制层用静态注册表把官方 CLI 与 Adapter 关联，Gateway 提供模型、Chat 和 Responses 文本接口。额度独立保留来源及观测时间，Codex 主/次窗口在 `quota_buckets` 中分开存储，应用/路由UI共用多窗口；内部旧路由策略仍兼容单快照。当前v0.2.0基线见[封版记录](../releases/V0_2_0_FREEZE.md)，connection instance等后续工作见[TODO](../TODO.md)；[产品对齐计划](../LOCALMUX_PRODUCT_ALIGNMENT_PLAN_2026-09-28.md)仅作历史决策依据。

调用从HTTP入站起单调计时1200秒，CLI空闲快照30–1200秒；API health兼容报告服务名/编译版本与执行限值。默认Key沿用开发期Unix0600文件/Windows OS-store实现，细节与未验边界见安全文档；没有新Schema迁移。会话元数据、显式正文录制、Responses续接与六格式本机导出仍分别管理，不读取外部私有历史。
