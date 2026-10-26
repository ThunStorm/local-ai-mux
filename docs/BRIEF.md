# LocalAiMux 产品说明

LocalAiMux v0.2.0是Tauri桌面应用，将用户显式连接的官方AI运行时通过本机OpenAI兼容文本API提供给其他客户端。主流程为发现应用、显式连接、读取真实型号/能力、启动本机API并复制配置；不扫描私有凭据，不通过第三方GUI自动化接入。

macOS开发基线包括按Key/连接/模型授权、Chat与Responses、typed流式元数据、额度窗口、显式正文录制与六格式筛选导出、分层超时诊断。监听仍为127.0.0.1:8317，CLI空闲30–1200秒（默认180、保留已保存值），Gateway从入站起总期1200秒；其他独立限制见[Streaming](api/STREAMING.md)。

当前代码/验证与封版见[v0.2.0](releases/V0_2_0_FREEZE.md)，运行现场见[交接](HANDOFF.md)，后续工作见[TODO](TODO.md)。[初始项目计划](LOCALMUX_PROJECT_PLAN.md)和[产品对齐计划](LOCALMUX_PRODUCT_ALIGNMENT_PLAN_2026-09-28.md)属于历史规划，不能作为全部已实现的证明。签名发行、其他平台原生、精确上游质量及远端取消各自独立验收。
