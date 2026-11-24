# Qoder / WorkBuddy 积分读取与录制展示调研

核查日期：2026-10-16。结论依据公开文档与现有代码；没有新连接、读取私有登录态、调用余额服务或发送真实生成。以下是实施路径，不代表功能已经接入或真实验收通过。

| 产品 | 账户剩余积分 | 单次请求 / 会话消耗 | 当前判断 |
| --- | --- | --- | --- |
| Qoder | 官方 Agent SDK `getUsageInfo()` / `get_usage_info()` 提供套餐、加量包、组织资源包 | Assistant usage 提供请求积分；Result / usage.session 提供当前 CLI 会话累计积分 | 有公开实现路径，需要兼容性验证与 Adapter 接入 |
| WorkBuddy | 英文 Open API 文档公开个人积分接口，需 OAuth 与指定 scope | ACP 文档提及 usage，但未定义可确认的会话 Credits 字段 | 余额有条件可接；会话积分仍未知 |

## Qoder

来源：[Cost and usage](https://docs.qoder.com/cli/sdk/cost-usage)、[TypeScript reference](https://docs.qoder.com/cli/sdk/references-typescript)、[Python reference](https://docs.qoder.com/cli/sdk/references-python)、[SDK authentication](https://docs.qoder.com/cli/sdk/authentication)。这些公开文档已超出原 QODER.md 仅描述交互式 `/usage` 的旧调查范围。

- 账户：`getUsageInfo()` 返回 `userQuota`、`addOnQuota`（total/used/remaining/percentage/unit）与 `orgResourcePackage`（cap/used/remaining/percentage/available/unit）。分别映射独立额度桶，不能把组织共享额度和个人额度直接相加。`expiresAt` 文档为 Unix 毫秒且是到期时间，不直接当桶的秒级 reset_at。
- 请求：`AssistantMessage.message.usage`（TS）/ `AssistantMessage.usage`（Python）的 `credits` 为优惠后积分，`original_credits` 为优惠前积分，`billable` 为是否计入积分消耗。一次 Agent 任务可能包含多个模型请求；需要按供应商请求标识去重并确认覆盖完整，不能把每个文本增量当一次计费。
- 会话：Result 的 `total_credits` 为当前 CLI 会话累计值；TS Result `modelUsage[model].credits` 与 `getUsageInfo().session.model_usage` 命名不同。多个 Result 不相加，请求积分与会话累计也不相加。Result 的 `usage` 不作为请求积分来源。
- 未知：字段可选，旧 CLI 可能不支持，余额失败时仍可能只有 session；缺失不能补零。剩余积分用账户桶，不用会话消耗反推。`total_cost_usd` 不等于积分。
- 授权：SDK 公开支持 `qodercliAuth()` 复用用户本机官方 CLI 登录，也支持用户显式提供 PAT；仍需用户在应用中显式连接。不由 LocalAiMux 自行读取令牌文件。
- 无生成查询：官方例子使用不产生 prompt 的异步输入，等待初始化后读取 getUsageInfo，最后关闭。后续应使用正式 SDK/公开契约，设置超时、进程清理及最小工具权限，不猜测内部 HTTP 路径。

本机仅执行 `qoder --version`，报告 1.1.64；未证明该版本支持新 SDK 字段。现有 Rust Adapter 使用 headless stream-json，通用 CLI parser 尚未映射这些积分；不能据 SDK 文档断言当前安装版已自动采集。后续先在隔离 fixture 覆盖账户桶/只含 session/null/零值/重复累计/旧版本，再经用户在 UI 显式刷新验证余额，真实消耗等待用户显式发送的会话验收。Qoder CN 必须单独核对公开契约与版本，不跨产品复用结论。

## WorkBuddy

来源：[英文 Open API Reference](https://open.workbuddy.cn/en/docs/openapi)、[中文 Open API](https://open.workbuddy.cn/docs/openapi)、[查看套餐与用量](https://www.codebuddy.cn/docs/workbuddy/Usage)。本次直接抓取公开页面核实：英文页有 Read Personal Credits，中文页仍主要描述个人资料/手机号，存在语言版本差异，接入前需确认服务实际开放情况。

- 英文页定义 `GET https://www.workbuddy.cn/openapi/v2/credit`，Bearer 授权，scope=`user.credit.readable`，响应 `total_capacity_size` 与 `total_capacity_used`。文档限定个人用户，统计有效及已用尽资源包，不拆套餐/充值/赠送；剩余量可按同一响应 total-used 计算（这是本地算术派生值，需标明来源并校验非负、used≤total），不能宣称服务端直接返回 remaining。
- 前提是开放平台注册开发者、创建第三方应用、取得 clientId/clientSecret、配置回调与授权列表，用户显式 OAuth 同意。公开 token 交换要求 client_secret 且注明不得暴露在客户端；未找到可直接供分发桌面应用使用的免密 PKCE 契约。因此不能硬编码共享 secret，需可信后端或厂商明确支持的桌面授权方案后再实施。现有 WorkBuddy 桌面登记不等于获得该授权。
- ACP 云任务 `session/prompt` 的终态有 stopReason + usage，但该文档没有定义可验证的积分计量字段/范围，不能把 usage 或 token 数直接标成 Credits。它也不是读取所有桌面历史会话积分的公开保证。官方网页可按任务/模型查看明细，尚未找到可对应本地录制 conversation_id 的公开消耗明细接口。
- 开始/结束账户余额差值会混入其他设备、并发任务、补充/过期和结算延迟。即使后续提供“期间账户变化”，也不能冠以“本会话官方消耗”；本轮不实施该近似。独立 CodeBuddy Code 的身份/积分不能充当 WorkBuddy。

## 后续展示与验收建议

账户额度继续使用 quota_buckets；录制展示需另建有界、可空的积分元数据，明确 connection/provider/session/request 标识、unit=credits、来源、request/last_turn/session_total 范围、观测时间及是否完整。新功能默认只保存允许录制的新记录，尊重 store:false 与清理语义，不回填历史。续接会话的累计值取最近可信终态；缺少可信同一会话基线时本轮消耗未知，不用累计值冒充本轮。会话结束前可标待结算。UI 应分别显示“本次调用消耗”和“上游会话累计”，余额单独显示。

建议先做 Qoder 正式 SDK 能力验证和请求/累计积分映射，再做 WorkBuddy 授权方案与余额读取；WorkBuddy 会话 Credits 待厂商公开字段、范围及会话关联后再实现。验收必须同时覆盖多轮不重复累计、失败/取消/缺失不补零、显式零值、账户切换隔离、store:false、旧记录不变及录制关闭；不以离线 fixture 代替真实计费核对。
