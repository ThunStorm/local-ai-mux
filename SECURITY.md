# 安全说明

LocalMux 是本机桌面应用，本地 API 默认只监听 `127.0.0.1:8317`。API Key 只显示一次，数据库存哈希；默认 Key 无 Agent 调用权限。只有在桌面界面明确选择“只读 Agent”后，Key 才可调用已连接的 Agent。上游运行时自行执行工具权限，LocalMux 不承诺它无法验证的沙箱能力。

账号与 Provider 密钥应由用户主动连接并放入 macOS Keychain / Windows Credential Manager。不得写入 SQLite 明文、日志、仓库或故障报告。安全问题请私下联系维护者后再公开披露。
