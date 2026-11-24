# LocalAiMux 55 型号能力与限制归因盘点

采样：北京时间 2026-10-06 19:17；只读调查，未发起模型生成。配套[设计说明书](CAPABILITY_NEGOTIATION_DESIGN.md)区分当前行为与待实施规格；[机器可读证据](capability-audit-20261006.json)保存脱敏目录、版本、CLI 旗标与 Codex 官方元数据。本文件不是 55 型号真实质量 Benchmark。

此处 C0 与限制归因为 19:17 旧安装版的历史观测，不覆盖后续实施行为；当前能力及 55/55 复核见[实施验收](CAPABILITY_NEGOTIATION_IMPLEMENTATION_2026-10-06.md)，不得把历史 C0 当作新版本的有效能力。

## 1. 盘点覆盖与证据等级

两个本机目录 `/v1/models`、`/api/chat/models` 均为 55 个唯一 ID、集合一致：Codex 7、Qoder 17、Qoder CN 14、CodeBuddy 17。当前 Key 可见的全部型号逐项列在第 5 节；数据库四个已保存连接一致。不存在将已安装但未连接的 MiMo/Claude/Gemini/WorkBuddy 等加入本表的情况，不断言该 Key 看不到的型号不存在。

| 证据 | 本次实际获得 | 不能替代的证据 |
| --- | --- | --- |
| Mux 运行时能力 | 全部 55 项有效 chat 描述，相同 C0 | 不是底层模型自身能力 |
| 已安装公开接口 | CodeBuddy 2.161.0、Qoder/CN 1.1.64 的 --version/--help | 有旗标不等于每个型号、模式、数值实际生效 |
| Codex 本机公开协议 | codex-cli 0.160.0 的 generate-json-schema；初始化后只读 model/list，7 型号，无分页 | 没有 thread/start、turn/start 或生成，不证明 Schema/视觉等已真实运行 |
| 官方 SDK 文档 | Qoder/CN ModelInfo/getAvailableModels 与 CodeBuddy SDK 思考等参数契约 | 本轮没有完成这些 SDK 与已安装 CLI 的配对，不能写 48 型号精确枚举/窗口为已读取 |
| 厂商原生 API 文档 | 如 hy3 公开 TokenHub 参数、DeepSeek 原生参数 | CodeBuddy/Qoder 服务出口不是同一个 API；同名/别名不能覆盖当前连接能力 |
| Mux 源码 | Gateway 请求型、single_turn 目录/校验、进程参数、结果封装 | 不替代已运行版本的真实推理；当前元数据由运行时另行复核 |

底层参数/质量无法通过本次只读路径取得时明确 UNKNOWN，不猜默认窗口、输出上限、费用、模型提供商或推理是否可关。**清单完整和数据完整是两件事：型号 55/55；底层型号参数有证据的维度全部列出，其余保留缺口。**

## 2. 当前公共限制来自哪里

| 现象 | 实际发生层/原因 | 是否可归因于模型 |
| --- | --- | --- |
| max_tokens 400 | Gateway 可解析字段；四个 Adapter 把所有型号配置为单轮模板并声明 max_tokens UNSUPPORTED；Gateway 校验在启动 CLI 前拒绝 | **不能**；此请求未到 CodeBuddy/hy3 推理。Qoder CLI 另有公开输出限制旗标，表明还有 Mux 映射缺口 |
| temperature/top_p 400 | 同一 Adapter 静态能力声明和 Gateway 参数校验；未建立公开采样映射 | **不能**；CLI/订阅出口是否能传与原生模型是否能采样需分开 |
| system、多消息 400 | Adapter 的 single_user_prompt 只接受单 user；不是根据每个底层型号查询后作出的裁定 | **不能**；CodeBuddy/Qoder 有 system prompt，Codex 公开协议有指令字段 |
| response_format / thinking / reasoning_effort 等 400 | 顶层 ChatRequest 使用 deny_unknown_fields，目前 schema 没有这些字段 | **不能**；请求反序列化即拒绝，不证明底层能力 |
| 图片/音视频/文件输入被拒绝 | Gateway 强制 content 字符串，Adapter 当前文本输入；未建立公开媒体映射 | **不能**；Codex 7 型号官方 metadata 声明 text/image，Mux 没有开放其图片输入 |
| history=UNSUPPORTED | 当前 Chat 无任意历史映射，静态模板声明；与 native resume 分离 | **不能**；三个 Agent 客户端均有会话恢复入口，但恢复并不等同任意历史导入 |
| usage=0 / finish_reason=stop | Gateway 非流式封装固定值；NormalizedResponse/NormalizedStream 没有完整用量/结束元数据 | **不能**；Codex 有 usage 事件，其他公开 SDK 有结果用量结构。当前缺少解析/传递 |
| 所有型号看起来一样 | ModelDescriptor.chat 均采用 single_turn，不保留底层已有的差异 | **不能**；Codex 默认 effort/合法枚举已证实不同 |

源码位置：[ChatRequest](../../crates/api-types/src/lib.rs)、[Gateway](../../crates/gateway/src/lib.rs)、[共享请求/响应类型](../../crates/adapter-sdk/src/lib.rs)、[Codex Adapter](../../crates/adapters/codex/src/lib.rs)、[Qoder Adapter](../../crates/adapters/qoder/src/lib.rs)、[CodeBuddy Adapter](../../crates/adapters/codebuddy/src/lib.rs)、[CLI 流解析](../../crates/adapter-sdk/src/cli.rs)。

对 hy3 应分开下结论：公开混元 TokenHub 资料列有 max_tokens 和结构化输出场景；CodeBuddy 的当前公开 CLI 不提供已确认的回答 Token 上限旗标；Mux 当前 max_tokens 明确拒绝。因此属于“Mux 未映射/公开出口证据不足”，**不能写成 hy3 不支持 max_tokens**。[腾讯官方资料](https://cloud.tencent.cn/document/product/1823/132252)的整页本轮读取失败，结论仅使用官方检索内容，不照此冻结 hy3-x 等别名约束。

## 3. 官方客户端接口能力与 Mux 缺口

### 3.1 Codex 0.160.0（桥代码 C）

本机公开 schema 定义 thread/start 的 baseInstructions/developerInstructions、turn/start 的 effort/outputSchema 及 text/image/localImage 输入、thread/tokenUsage/updated 的 last/total/modelContextWindow 和 input/output/reasoning/cache 用量字段。model/list 给出了下表 7 型号的具体 inputModalities 与推理档位。这些内容目前未被 Mux 的单轮文本目录与响应类型保留；输出上限/采样控制本次没有取得稳定 per-request 出口，不声称可提供。

Native thread resume 和 Chat history 导入不是同一能力。App Server 提供输出 Schema 的事实也不等于七个型号已完成真实结构化输出验收。指令字段的替换/追加和安全边界须在后续适配中核实。[官方 App Server](https://learn.chatgpt.com/docs/app-server)

### 3.2 Qoder 与 Qoder CN 1.1.64（桥代码 Q / QN）

两份已安装 --help 均实际列出：--system-prompt、--append-system-prompt、--input-format stream-json、--max-output-tokens、--reasoning-effort、--thinking（auto/adaptive/enabled/disabled）、--thinking-budget、--context-window、--resume/--session-id、--tools、--strict-mcp-config 和配置来源控制。当前 Mux 只传单文本 Prompt、模型/会话及工具控制，未映射 system/输出/思考等能力。

官方 SDK 的 getAvailableModels 返回 ModelInfo，包含 context_config/thinking_config 及可选的价格/可用性信息；应以连接自己的 SDK 元数据按型号读取，不能套用 help 的通用参数到全部 31 型号。Result/ModelUsage 也有用量与上下文等字段。公开 TypeScript model policy 可调整 contextWindow/reasoningEffort；任意 modelRequestPatches 不作为无白名单通用透传通道。本轮 SDK 未配对可用，当前这些型号的窗口、输出上限数值、合法思考档位、关闭思考与多模态有效性仍 UNKNOWN。[公开 CLI](https://docs.qoder.com/cli/cli-reference)、[Qoder SDK](https://docs.qoder.com/cli/sdk/references-typescript)、[Qoder CN SDK](https://docs.qoder.cn/en/cli/sdk/references-typescript)

### 3.3 CodeBuddy 2.161.0（桥代码 B）

已安装 --help 实际列出 --system-prompt/--system-prompt-file/--append-system-prompt、--input-format stream-json、--json-schema、--resume/--continue/--session-id、--tools/--strict-mcp-config/--setting-sources，以及 17 个模型 ID。不能把 --output-format json（CLI 事件包装）当成模型 JSON mode；--json-schema 的描述是结构化输出验证，需核实实际保证、字段和失败行为。

本次公开 CLI help 没有找到 max-output-tokens/max-tokens/temperature/top-p；结论只是当前出口未找到，不宣称模型不支持。公开 CodeBuddy SDK 另有 thinking/effort/output_format/结果用量等候选控制，但本轮未安装或配对 SDK，不能把“最新文档有选项”算作 2.161.0 已逐型号验证。结构化输入和 resume 可用于多轮运行，但不能直接推断任意 Chat assistant 历史导入和角色层级正确。[公开 CLI](https://www.codebuddy.cn/docs/cli/cli-reference)、[官方 Python SDK](https://www.codebuddy.cn/docs/cli/sdk-python)、[SDK 范围](https://www.codebuddy.cn/docs/cli/sdk)

### 3.4 全部四种桥的公开能力对照

| 维度 | C：Codex | Q/QN：Qoder 两版 | B：CodeBuddy | Mux 当前有效 |
| --- | --- | --- | --- | --- |
| 自定义高优先级指令 | schema 有 base/developerInstructions | CLI 有 system prompt | CLI 有 system prompt | 不支持，未映射 |
| 回答 Token 上限 | 本轮未确认稳定出口 | CLI 有旗标；型号范围/口径待读 | CLI 未找到；SDK/型号范围未知 | 所有型号不支持 |
| temperature/top_p | 未确认稳定出口 | CLI help 未列采样；SDK任意 patch 不当作支持 | CLI help 未列采样 | 所有型号不支持 |
| 思考/effort | 7 型号明确枚举，见逐项表 | CLI 控制存在，逐型号值未知 | SDK 文档有控制；CLIhelp不提供同等证据 | Gateway 无字段、Adapter 未映射 |
| 输出 Schema | turn/start.outputSchema | 本轮未证实 | --json-schema / SDK 候选 | Gateway 无字段、Adapter 未映射 |
| 多轮/原生续接 | native thread resume | CLI resume/session | CLI resume/session | Responses 可走 native resume；Chat history仍不支持，真实多轮本轮未验收 |
| 输入模态 | 7 型号声明 text/image | 精确型号声明未读取 | 精确型号声明未读取 | 仅纯文本 |
| usage/结束原因 | 公开事件有 Token metadata | SDK 结果有 usage/model_usage，当前未读 | SDK 结果结构有候选字段，需版本核对 | 非流式固定占位；Responses usage=null |
| 真正业务生成质量 | 本轮未测 | 本轮未测 | 本轮未测 | 本轮未测 |

“未映射”说明改造机会，不等于可立刻改为 SUPPORTED；功能需要版本、型号和测试证据。非流式/SSE、session ID 捕获、只读/工具限制是当前桥已有的独立功能，其真实效果不由目录声明担保。

## 4. C0 与逐型号表的读法

C0 是运行时全部型号的 **当前 Mux 接口描述**：stream=true；history/system/temperature/top_p/max_tokens 为 UNSUPPORTED；三个参数 min=0、max=null 为占位，不能理解为“数值 0 允许”。额外字段和图片等受第 2 节 Gateway 限制。C0 不赋予底层模型原生能力结论。

表内“桥”对应第 3 节公开接口路径；这是安装版本级能力，不是逐型号全部成功。C 的 input/effort 是 model/list 实际元数据；Q/QN/B 的具体型号参数未有可用的动态能力证据，均明确 U。U=UNKNOWN（未取到/未核实），不能等同不支持。

下列全部型号的输出/上下文数值、采样有效性、原生 JSON 保证、业务质量、实际可用余额/费用以及当前是否能关闭思考，除表内声明外均 U。本轮没有读取任何用户 Prompt、历史对话或供应商凭据来补这些信息。

## 5. 当前全部型号：55/55

| 序号 | 精确 ID | 公开桥 | 逐型号已读取的输入/推理信息 | 输出/窗口/其他精确参数 | 当前 Mux |
| --- | --- | --- | --- | --- | --- |
| 1 | `codex/gpt-6.1-sol` | C | text/image；low,medium,high,xhigh,max,ultra；默认 low | U | C0 |
| 2 | `codex/gpt-6-astra` | C | text/image；low,medium,high,xhigh,max,ultra；默认 low | U | C0 |
| 3 | `codex/gpt-6-sol` | C | text/image；low,medium,high,xhigh,max,ultra；默认 medium | U | C0 |
| 4 | `codex/gpt-6-luna` | C | text/image；low,medium,high,xhigh,max；默认 medium | U | C0 |
| 5 | `codex/gpt-5.6-sol` | C | text/image；low,medium,high,xhigh,max,ultra；默认 low | U | C0 |
| 6 | `codex/gpt-5.6-terra` | C | text/image；low,medium,high,xhigh,max,ultra；默认 medium | U | C0 |
| 7 | `codex/gpt-5.6-luna` | C | text/image；low,medium,high,xhigh,max；默认 medium | U | C0 |
| 8 | `qoder/Auto` | Q | U；需公开型号元数据 | U | C0 |
| 9 | `qoder/Ultimate` | Q | U；需公开型号元数据 | U | C0 |
| 10 | `qoder/Performance` | Q | U；需公开型号元数据 | U | C0 |
| 11 | `qoder/Efficient` | Q | U；需公开型号元数据 | U | C0 |
| 12 | `qoder/Sonus` | Q | U；需公开型号元数据 | U | C0 |
| 13 | `qoder/Cantus` | Q | U；需公开型号元数据 | U | C0 |
| 14 | `qoder/Qwen3.8-Max` | Q | U；需公开型号元数据 | U | C0 |
| 15 | `qoder/Qwen3.8-Flash` | Q | U；需公开型号元数据 | U | C0 |
| 16 | `qoder/Qwen3.7-Max` | Q | U；需公开型号元数据 | U | C0 |
| 17 | `qoder/Qwen3.7-Plus` | Q | U；需公开型号元数据 | U | C0 |
| 18 | `qoder/Kimi-K3` | Q | U；需公开型号元数据 | U | C0 |
| 19 | `qoder/Kimi-K2.8-Preview` | Q | U；需公开型号元数据 | U | C0 |
| 20 | `qoder/GLM-5.3` | Q | U；需公开型号元数据 | U | C0 |
| 21 | `qoder/GLM-5.3-Flash` | Q | U；需公开型号元数据 | U | C0 |
| 22 | `qoder/DeepSeek-V4-Pro` | Q | U；需公开型号元数据 | U | C0 |
| 23 | `qoder/DeepSeek-Flash` | Q | U；需公开型号元数据 | U | C0 |
| 24 | `qoder/MiniMax-M3` | Q | U；需公开型号元数据 | U | C0 |
| 25 | `qoder-cn/Auto` | QN | U；需公开型号元数据 | U | C0 |
| 26 | `qoder-cn/Qwen3.8-Max` | QN | U；需公开型号元数据 | U | C0 |
| 27 | `qoder-cn/Qwen3.8-Flash` | QN | U；需公开型号元数据 | U | C0 |
| 28 | `qoder-cn/Qwen3.7-Max` | QN | U；需公开型号元数据 | U | C0 |
| 29 | `qoder-cn/Qwen3.7-Plus` | QN | U；需公开型号元数据 | U | C0 |
| 30 | `qoder-cn/Qwen3.7-Flash` | QN | U；需公开型号元数据 | U | C0 |
| 31 | `qoder-cn/DeepSeek-V4-Pro` | QN | U；需公开型号元数据 | U | C0 |
| 32 | `qoder-cn/DeepSeek-Flash` | QN | U；需公开型号元数据 | U | C0 |
| 33 | `qoder-cn/GLM-5.3` | QN | U；需公开型号元数据 | U | C0 |
| 34 | `qoder-cn/GLM-5.3-Flash` | QN | U；需公开型号元数据 | U | C0 |
| 35 | `qoder-cn/GLM-5.2` | QN | U；需公开型号元数据 | U | C0 |
| 36 | `qoder-cn/Kimi-K3` | QN | U；需公开型号元数据 | U | C0 |
| 37 | `qoder-cn/Kimi-K2.8-Preview` | QN | U；需公开型号元数据 | U | C0 |
| 38 | `qoder-cn/MiniMax-M2.7` | QN | U；需公开型号元数据 | U | C0 |
| 39 | `codebuddy/hy4-preview` | B | U；需公开型号元数据 | U | C0 |
| 40 | `codebuddy/hy3` | B | U；需公开型号元数据 | U | C0 |
| 41 | `codebuddy/hy3-x` | B | U；需公开型号元数据 | U | C0 |
| 42 | `codebuddy/space-bunny` | B | U；需公开型号元数据 | U | C0 |
| 43 | `codebuddy/deepseek-v4.1-flash` | B | U；需公开型号元数据 | U | C0 |
| 44 | `codebuddy/glm-5.3` | B | U；需公开型号元数据 | U | C0 |
| 45 | `codebuddy/glm-5.3-flash` | B | U；需公开型号元数据 | U | C0 |
| 46 | `codebuddy/glm-5.2` | B | U；需公开型号元数据 | U | C0 |
| 47 | `codebuddy/glm-5.1` | B | U；需公开型号元数据 | U | C0 |
| 48 | `codebuddy/glm-5v-turbo` | B | U；需公开型号元数据 | U | C0 |
| 49 | `codebuddy/minimax-m3` | B | U；需公开型号元数据 | U | C0 |
| 50 | `codebuddy/minimax-m2.7` | B | U；需公开型号元数据 | U | C0 |
| 51 | `codebuddy/kimi-k3-1` | B | U；需公开型号元数据 | U | C0 |
| 52 | `codebuddy/kimi-k2.8-preview` | B | U；需公开型号元数据 | U | C0 |
| 53 | `codebuddy/kimi-k2.7` | B | U；需公开型号元数据 | U | C0 |
| 54 | `codebuddy/kimi-k2.6` | B | U；需公开型号元数据 | U | C0 |
| 55 | `codebuddy/deepseek-v4-pro` | B | U；需公开型号元数据 | U | C0 |

表内型号保留大小写、前缀和默认值，不把 Qoder 性能档位/Sonus/Cantus、CodeBuddy space-bunny 等映射到猜测厂商。Qwen/GLM/Kimi/DeepSeek/MiniMax/Hy 的名称不能证明该订阅连接等价于厂商原生 API；不从答案自述判断实际上游。

## 6. 全量盘点结论与剩余证据

1. 清单完整：55/55；运行时共同限制可归因到 Mux；三种客户端公开接口的能力差异已核实，不能继续把 C0 当作模型本体能力。
2. Codex 七型号获得精确输入模态、合法 effort 与默认值；当前 Mux 应保留这些已有数据，并分别记录“可报告”和“已可调用”。
3. Qoder/CN 31 型号的精确范围/默认值，应使用各自官方 SDK metadata；本轮没有配对可用 SDK，公共 CLI 名称表不包含这些值。CodeBuddy 17 型号的具体参数仍需其公开出口证据，尤其不要把 CLI help 没找到的控制写成底层不支持。
4. 后续无需逐个发真实生成才能补目录，但声明某个控制实际生效时必须有对应验证。先完成设计中的 M1/M2，真实验收按授权串行小样，不自动批量试 55 个型号。
5. 本次保留工作树已有改动，未重启/部署，也未重新证明其他任务在交接中记录的真实模型验收；此文仅报告本轮调查事实。
