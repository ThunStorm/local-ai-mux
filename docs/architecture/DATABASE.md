# 数据库

SQLite 通过版本化 migration 初始化：`0001` 设置、`0002` API Key 哈希与撤销、`0003` 显式添加的 CLI 路径、`0004` 路由策略与上游单值额度快照、`0005` Conversation/ProviderSession/Response ID 映射及路由事件、`0008` 显式添加的桌面应用清单、`0009` 多额度桶。外键已开启，续接映射以事务写入。

`added_applications` 只保存应用 ID 与安装路径；它不建立 `connections` 行，也不会把桌面登录态注册到 Gateway。
`0010` 将旧登记中按 `codebuddy` 合并的 WorkBuddy、CodeBuddy CN，以及按 `qoder` 合并的 Qoder CN，按安装路径迁到各自应用 ID；真实 CLI `connections` 原样保留，不跨产品迁移授权。
`crates/storage/build.rs` 使新增 migration 文件触发重新编译；存储测试断言最新版本为 10，避免安装包继续携带旧的编译期迁移清单。

研发版 MiMo 开放平台连接在 `connections.executable_path` 中存 `session-credential:mimo` 固定标记；真正 Key 仅在当前进程内存中。重启后标记行仍在，但连接不会恢复，用户需要显式重新输入 Key；旧 `os-credential:mimo` 行也不会触发系统凭据库读取。此连接不代表 MiMo Desktop 已登录。

手动创建的 API Key 明文只在创建时返回；默认本机 Key 从 2026-10-08 起跨重启/部署保留。SQLite 的 `api_keys` 仍只存哈希与权限，`settings.default_api_key_id` 存不透明 ID，`default_api_key_store_version=1` 标记已迁移。旧内存 Key 首次升级可换一次，此后不会自动轮换或恢复默认权限。macOS/Unix 的默认 Key 在应用数据目录 `local-api-keys/api.key.<id>` 中，目录 0700、文件 0600；Windows 使用独立 OS 凭据服务（实机未验证）。修改授权只更新原 ID 的 scopes/profile 并保留路由、连接与限流约束，不改哈希；显式重建以事务切换 ID/存储版本并撤销旧 Key。凭据缺失/损坏/已撤销时需明确恢复，不补造新 Key。MiMo/第三方凭据策略仍按上段，不因本机 Key 持久化改变。

供应商 Key、提示词和回答正文不进入 SQLite；旧 Keychain 条目不会自动读取或删除。凭据文件不属于数据库备份/日志/诊断导出内容，备份恢复需考虑 Key 与元数据的一致性。额度空值表示未知，不能补成 0；Mock Gateway 使用隔离数据库与合成 Key。正式产品权限与安全存储任务见[安全 TODO](SECURITY.md)。

`quota_buckets` 以 `(connection_id, bucket_id)` 分别保存官方窗口，旧 `quota_snapshots` 迁移到 `legacy` 桶并继续服务旧路由兼容。非空官方刷新在事务中替换该连接的桶；刷新失败保留旧观测时间，界面超过 10 分钟标过期，不相加成余额。
