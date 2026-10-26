# Xiaomi MiMo API Adapter

MiMo Desktop 的登录态没有公开、稳定的本地调用契约。LocalAiMux 只通过用户在 Discover 页显式提交的 MiMo 开放平台 API Key 连接 `https://api.xiaomimimo.com/v1`，使用公开的 OpenAI 兼容模型目录和 Chat Completions。连接先读取模型目录，不发送计费推理请求；成功后 Key 存在系统凭据库，SQLite 只保存固定引用 `os-credential:mimo`。断开时删除该凭据。

模型目录来自上游 `GET /models`，所有费用分类均为 `UNKNOWN`。桌面应用的订阅、开放平台 Token Plan 与 API 账单不能混为一个额度。未找到稳定公开的账户余额接口时，`get_quota` 返回 None；界面显示未知并给出官方控制台入口。

离线验证使用 `localmux-adapter-sdk` 的 HTTP mock；真实 Key、账户权限、模型列表和计费需要用户在 LocalAiMux 中亲自连接后验收。正式使用前还需验证 Keychain 恢复、断开删除和官方 SDK 端到端请求。
