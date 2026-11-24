# 额度模型

`QuotaSnapshot` 区分 `PROVIDER_REPORTED`、`ESTIMATED`、`UNKNOWN`。字段包括连接、剩余/总量、单位、重置时间和观测时间；缺失值保持空，不当作 0。只有 10 分钟内的上游报告且剩余大于 0，才能证明 Reserve 可用。

桌面启动、连接完成、每 5 分钟及用户刷新时尝试调用 Adapter 的 `get_quota`；当前 Adapter 默认返回 None，界面显示“官方额度未知”。每连接设置至少 60 秒刷新间隔；429 后针对该连接触发受控刷新。只有 `PROVIDER_REPORTED` 快照写入 SQLite；本地 usage 统计不能伪装成官方余额。

额度页按每个已连接服务列出状态。没有机器可读官方余额时，显示 `UNKNOWN` 与空数值，不把缺失当作零。Qoder `/usage`、WorkBuddy 积分界面及 MiMo 控制台余额目前没有已验证的公开机器可读余额契约；不能抓取桌面登录态或网页私有接口来填充快照。

Codex Adapter 从公开 App Server `account/rateLimits/read` 读取 `codex` 用量窗口，仅在 `ordinaryUsageAllowed=true` 且 `usedPercent` 合法时上报剩余百分比、窗口时长及重置时间。此值是用量窗口百分比，不是 Credits 或货币余额。超过 10 分钟的快照在桌面页降为未知。
