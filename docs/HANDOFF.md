# LocalAiMux 当前交接

当前版本为 v0.3.0，范围见[版本说明](releases/V0_3_0.md)。[架构总览](architecture/OVERVIEW.md)提供模块、调用与存储入口，[TODO](TODO.md)记录正式发行及其他后续工作。

- 数据与日志已提供可配置目录、重启快照迁移、单实例锁及脱敏日志。迁移兼容 Schema 13，默认数据及本机 Key 锚点沿用原位置。恢复步骤见[数据治理](architecture/DATA_LIFECYCLE.md)。
- Gateway 已支持可分类首包超时和连接冷却。Qoder CLI 有界 stderr 用于归因。历史真实样本曾出现上游内部首包超时而 Gateway 返回通用 502，新归因仍待真实样本验证。接口语义见[错误模型](api/ERROR_MODEL.md#首包超时与连接冷却)。
- 源码发布包含离线 fixture。历史本机验证摘要见 [v0.2.0 封版](releases/V0_2_0_FREEZE.md)和[历史交接](history/HANDOFF_THROUGH_2026-10-22.md)。当前提交的跨平台验证以对应 GitHub Actions 结果为准。
