# 0001 — 当前发布范围聚焦桌面订阅接入

状态：采纳，2026-09-28。

LocalAiMux 的当前用户路径是「扫描已安装应用 → 用户显式添加官方 Bridge Runtime → 查看实际报告的模型及额度 → 复制本机 OpenAI 兼容文本 API 配置」。扫描和应用登记都不是已连接的证据；LocalAiMux Key 仅认证本机入口，不产生上游额度。MiMo 开放平台 API Key 属单独的 BYOK 连接，不能当作 MiMo Desktop 订阅。

当前界面以真实模型 ID 为主。已有 `free/cheap/balanced/best` 路由代码、持久数据和旧 Key 权限继续兼容，但不作为新手默认流程；额度未知保持 UNKNOWN，不推断免费或余额。官方接口、用户显式连接、只读 Agent 权限、OS 凭据库和 `127.0.0.1:8317` 边界不变。真实上游、Windows 安装和发行验收分别记录，Mock 不替代真实证据。

依据：[产品对齐计划](../../LOCALMUX_PRODUCT_ALIGNMENT_PLAN_2026-09-28.md)。实施状态以 [交接记录](../../HANDOFF.md) 为准。
