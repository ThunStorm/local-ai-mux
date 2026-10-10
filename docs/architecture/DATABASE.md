# 数据库

SQLite 通过版本化 migration 初始化：`0001` 设置、`0002` API Key 哈希与撤销、`0003` 显式添加的 CLI 路径、`0004` 路由策略与上游单值额度快照、`0005` Conversation/ProviderSession/Response ID 映射及路由事件、`0008` 显式添加的桌面应用清单、`0009` 多额度桶。外键已开启，续接映射以事务写入。

`added_applications` 只保存应用 ID 与安装路径；它不建立 `connections` 行，也不会把桌面登录态注册到 Gateway。
`0010` 将旧登记中按 `codebuddy` 合并的 WorkBuddy、CodeBuddy CN，以及按 `qoder` 合并的 Qoder CN，按安装路径迁到各自应用 ID；真实 CLI `connections` 原样保留，不跨产品迁移授权。
`crates/storage/build.rs` 使新增 migration 文件触发重新编译；存储测试断言最新迁移版本（当前为 13），避免安装包继续携带旧的编译期迁移清单。

研发版 MiMo 开放平台连接在 `connections.executable_path` 中存 `session-credential:mimo` 固定标记；真正 Key 仅在当前进程内存中。重启后标记行仍在，但连接不会恢复，用户需要显式重新输入 Key；旧 `os-credential:mimo` 行也不会触发系统凭据库读取。此连接不代表 MiMo Desktop 已登录。

手动创建的 API Key 明文只在创建时返回；默认本机 Key 从 2026-10-08 起跨重启/部署保留。SQLite 的 `api_keys` 仍只存哈希与权限，`settings.default_api_key_id` 存不透明 ID，`default_api_key_store_version=1` 标记已迁移。旧内存 Key 首次升级可换一次，此后不会自动轮换或恢复默认权限。macOS/Unix 的默认 Key 在应用数据目录 `local-api-keys/api.key.<id>` 中，目录 0700、文件 0600；Windows 使用独立 OS 凭据服务（实机未验证）。修改授权只更新原 ID 的 scopes/profile 并保留路由、连接与限流约束，不改哈希；显式重建以事务切换 ID/存储版本并撤销旧 Key。凭据缺失/损坏/已撤销时需明确恢复，不补造新 Key。MiMo/第三方凭据策略仍按上段，不因本机 Key 持久化改变。

供应商 Key 不进入 SQLite；提示词和回答仅在用户显式开启录制时写入独立正文表；旧 Keychain 条目不会自动读取或删除。凭据文件不属于数据库备份/日志/诊断导出内容，备份恢复需考虑 Key 与元数据的一致性。额度空值表示未知，不能补成 0；Mock Gateway 使用隔离数据库与合成 Key。正式产品权限与安全存储任务见[安全 TODO](SECURITY.md)。

`quota_buckets` 以 `(connection_id, bucket_id)` 分别保存官方窗口，旧 `quota_snapshots` 迁移到 `legacy` 桶并继续服务旧路由兼容。非空官方刷新在事务中替换该连接的桶；刷新失败保留旧观测时间，界面超过 10 分钟标过期，不相加成余额。

`0011` 增加独立的 `call_records` 元数据表与来源/状态/sequence 索引；该阶段迁移为 11（最新为 12）。它没有指向续接表的级联删除关系，清理调用记录不会破坏既有 Responses 映射。分页以自增 sequence 而非秒级时间为游标，查询绑定参数并固定最多 51 条；终态更新只匹配 in_progress，不覆盖已完成/启动恢复的结果。本机 open 在迁移后恢复上次运行遗留的记录为 interrupted，旧连接/Key/会话数据保留。迁移前必须备份并确认任务为 0。详细字段、空值与取消语义见[会话模型](SESSION_MODEL.md)。


`0012` 新增 `call_recording_settings`（单行开关/代次，初始关闭）与 `call_contents`（按 call_id 外键级联，独立 JSON 输入、回答与录制状态）。原元数据表、Key/连接和续接映射不改写；旧记录没有正文。开关关闭与终止当前录制同事务，条件更新禁止过期代次继续写；保存相同开关值不递增代次。元数据终态和正文 complete/partial 标记同事务，启动恢复保留部分正文。列表仍只查元数据，正文由独立 IPC 按需读取，详情不截断；清理元数据同步清理正文，续接映射仍保留。备份从本版本起可能包含用户已启用录制的正文，不能把数据库备份当作纯元数据诊断报告。

`0013` 在 call_records 添加可空 request_id 与受 8192 字节限制的 diagnostics JSON 及关联 ID 索引；最新 migration 为 13。没有新历史表、无正文复制、无旧记录回填。保留现有终态更新条件/事务，store:false 不写入新字段。升级需任务 0、完整性检查和 SQLite 一致备份；旧程序 SQLx 校验无法直接打开新版本数据库，回滚见[运行诊断](SESSION_MODEL.md#有界运行诊断2026-10-10--m3)。
