# 桌面交互

首启显示欢迎、扫描、连接、开启 API 的路径并自动做只读扫描。Discover 页展示检测结果、版本、路径、诊断和重扫；已实现的官方 CLI 可点击「添加」并走真实连接验证，完整的桌面应用可点击「添加应用」登记安装记录。登记后的状态明确显示「已添加应用（仅登记）」，不冒充已连接，也不读取桌面登录凭据。API 页可显式启停 Gateway、创建并一次性查看/复制 Key、撤销刚创建的 Key。

MiMo API 提供单独的显式 Key 输入；输入验证后 Key 仅写系统凭据库，SQLite 只记固定引用。MiMo 桌面登录与开放平台 API Key 分开。Qoder 桌面应用只有安装时，界面提供官方 CLI 安装说明。

Routing 页显示 free/cheap/balanced/best 的当前目标或“暂无符合条件的模型”；Reserve 进入 free、balanced 使用计费模型都需用户单独勾选并保存。best 需要 exact model ID，balanced 需要显式优先级。官方额度没有上游报告时显示“未知”。

Sessions 页展示已完成 Responses 调用产生的 Conversation ID、实际模型、供应商、最近活动时间及是否可续接；可查看路由事件，不展示提示词/回答正文。当前没有活跃 Run 取消控件，待 Phase 8 进程与 Run 管理补齐。界面默认中文，支持英文及系统/浅色/深色外观。
