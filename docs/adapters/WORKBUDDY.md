# WorkBuddy 接入调研（2026-10-06）

当前结论：还不能把 WorkBuddy 桌面订阅作为 LocalAiMux 的模型上游接入。独立 CodeBuddy Code 的公开 CLI 可接入，但它与 WorkBuddy 的账户、模型目录和额度不能混同；本轮不新增 WorkBuddy Adapter。

## 官方接口与方向

| 方案 | 官方证据 | 与当前需求的关系 |
|---|---|---|
| CodeBuddy Code headless CLI | [CLI 参考](https://www.codebuddy.cn/docs/cli/reference)、[无头模式](https://www.codebuddy.cn/docs/cli/headless) | 当前 CodeBuddy Adapter 使用的公开入口；不能单凭同名运行时证明 WorkBuddy 桌面账户共享 |
| CodeBuddy Code ACP / HTTP | [ACP](https://www.codebuddy.cn/docs/cli/acp)、[HTTP API](https://www.codebuddy.cn/docs/cli/http-api) | ACP 有公开协议；HTTP 标记 Beta，内部 `/internal/*` 无兼容保证。属于 CodeBuddy Code，未找到 WorkBuddy 桌面订阅对外暴露模型调用的稳定承诺 |
| WorkBuddy 企业智能体 API | [企业智能体](https://www.codebuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/CloudAgent) | 官方说明通过企业 API Key 集成企业 Agent，独立 Session/Runtime；这是另一授权通道，不等于个人桌面订阅。本轮没有企业 Key 或接口合同，因此未实施 |
| WorkBuddy 自定义模型 | [模型配置](https://www.codebuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Model) | WorkBuddy 可作为下游调用自定义 API；方向是 WorkBuddy → 模型服务。可由用户尝试把 LocalAiMux 配为它的模型服务，但不能用来把 WorkBuddy 内置模型导出到 LocalAiMux，且真实兼容性尚未验收 |
| WorkBuddy 连接器 | [连接器](https://www.codebuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Connector) | 给 WorkBuddy 增加外部数据/工具能力，不能据此导出订阅模型 |

官方文档部分直接抓取超时，以上内容由官方站点搜索索引交叉核对；ACP/环境变量页可直接读取。缺少公开承诺表示证据不足，不断言厂商永远不会支持。社区代理不作为本项目接口依据。

## 本机只读证据

本机应用 `/Applications/WorkBuddy.app` 为 5.7.6；包内 `Contents/Resources/app.asar.unpacked/cli/bin/codebuddy --version` 报 2.156.0。使用已安装 Node 24 执行公开 `--help` 可成功，帮助报告 9 个选项（其中包含 `auto`），所以“能显示帮助”不能当连接验收。

同一入口公开 `config get model` 返回 `MODULE_NOT_FOUND: ../dist/codebuddy`；包中有 `codebuddy-headless.js`、`codebuddy-lite-wb.mjs`，缺完整 `codebuddy.js`。启动器将 `--help` 路由到 headless，而 config 命令尝试加载完整产物，解释了两者表现不同。未设置包内私有 bundle/产品环境开关，未读取 WorkBuddy 配置、缓存模型目录、登录令牌或凭据，未发起 WorkBuddy 登录或推理。

独立 CodeBuddy Code 2.161.0 安装在 NVM Node 24 中：`config get model` 成功但返回空值，公开帮助报 17 个模型 ID；修复后的独立 Adapter 已通过只读目录检查。这些证据不能跨产品推广到 WorkBuddy。

## 后续接入门槛

若厂商提供面向第三方的 WorkBuddy 桌面官方 CLI/ACP/API，需核实可从用户显式授权获得身份、只读动态目录、费用/额度来源，以及能禁用工具/MCP/写入；然后单独实现 `workbuddy/` 身份与测试。企业 Agent API 应另行按企业授权合同评估。当前保留 WorkBuddy 桌面检测，不注册 Gateway，不读取私有缓存，不将 CodeBuddy 连接迁到 WorkBuddy。

## 2026-10-16 积分接口更新

英文开放平台文档已公开个人积分 `GET /openapi/v2/credit`（OAuth scope `user.credit.readable`），中文页内容尚不同步；账户总额/已用可用于派生剩余量。需要注册第三方应用及用户授权，当前 token 交换要求客户端不得暴露的 client_secret，桌面分发方案仍待确认；未找到能验证桌面会话 Credits 的公开字段。余额接口不代表 WorkBuddy 已可作为模型上游接入，现有登记和 CodeBuddy CLI 连接不继承此权限。详见[积分调研与录制方案](CREDITS_RESEARCH_2026-10-16.md)。
