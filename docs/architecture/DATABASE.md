# 数据库

SQLite 通过版本化 migration 初始化：`0001` 设置、`0002` API Key 哈希与撤销、`0003` 显式添加的 CLI 路径、`0004` 路由策略与上游额度快照、`0005` Conversation/ProviderSession/Response ID 映射及路由事件、`0008` 显式添加的桌面应用清单。外键已开启，续接映射以事务写入。

`added_applications` 只保存应用 ID 与安装路径；它不建立 `connections` 行，也不会把桌面登录态注册到 Gateway。

MiMo 开放平台连接在 `connections.executable_path` 中存 `os-credential:mimo` 固定引用；真正 Key 在系统凭据库的 `provider.mimo` 条目。此连接不代表 MiMo Desktop 已登录。

明文 API Key 只在创建时返回；供应商 Key、提示词和回答正文不进入 SQLite。额度表的空值表示未知，不能补成 0。测试用 Mock Gateway 使用内存数据库，避免在用户目录留下合成测试 Key。
