# MiMo Code CLI Bridge

连接使用 MiMo Code 公开的 [Capability API](https://github.com/XiaomiMiMo/MiMo-Code/blob/main/packages/opencode/src/skill/builtin/.bundle/mimocode-docs/reference/capability-api.md)。只有用户在应用内点击「添加并验证」，才从扫描到的 `mimo` CLI 启动 `mimo serve --pure --hostname 127.0.0.1 --port 0`；不读取桌面应用凭据，也不使用小米开放平台 API Key。

在隔离临时目录签发一次短时探测令牌，查询本机 `/v1/models` 后立即撤销；只允许当前目录报告的 `mimo/`、`xiaomi/` 型号进入第二个作用域受限令牌。令牌最长 30 分钟，明文仅留在 `HttpProvider` 内存中，不写数据库或日志。返回地址必须是 `http://127.0.0.1:<port>/v1`；断开时撤销令牌并结束子进程。应用重启后需再次由用户点击连接。

模型 ID 在 LocalAiMux 中是 `mimocode/<MiMo Code 原始 ID>`。目录只证明当前本机服务可报告该型号；真实推理、费用和额度均不据此宣称已验收。用户点击「测试此模型」才发出真实上游请求。开放平台 API Key 通道仍用 `mimo/` 前缀，与此 Bridge 独立。
