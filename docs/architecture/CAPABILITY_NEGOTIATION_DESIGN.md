# LocalAiMux 通用能力与客户端兼容设计说明书

版本：2026-10-06 设计基线。下文保留设计目标和工作包；M1/M2 最小文本/M3 与 M5 离线互操作已增量实施，实际实现、延期项和验证边界见[实施验收](CAPABILITY_NEGOTIATION_IMPLEMENTATION_2026-10-06.md)。不是全部目标已经实现；本轮未发起真实模型推理或新增连接/授权。逐型号事实与证据见[55 模型能力盘点](CAPABILITY_AUDIT_2026-10-06.md)。现有产品范围、显式连接和 Agent/Provider/Model/Account 身份边界仍受 [Brief](../BRIEF.md)、[Adapter 契约](ADAPTER_SYSTEM.md) 与 [API 契约](../api/OPENAI_COMPAT.md) 约束。

## 1. 设计结论

LocalAiMux 是面向任意客户端的本机通用模型 Gateway，不应成为至简 Pipeline 的分支。设计基线盘点时的 55 个型号都被套用 `ChatCapabilities::single_turn()`，因此对外共同的 UNSUPPORTED 主要描述 **Mux 当前未映射能力**，不能证明 CodeBuddy/Qoder/Codex 或其底层模型天生不支持。

公开接口证据已明确指出可补齐的能力：CodeBuddy CLI 的 system prompt、JSON Schema、结构化输入和 resume；Qoder/Qoder CN CLI 的 system prompt、最大输出、思考和窗口参数；Codex App Server 的指令、输出 Schema、推理档位及 usage 事件。补齐必须通过稳定公开接口并保留精确型号/版本约束；不使用私有供应商端点、内部配置解密、隐藏参数或 GUI 模拟。

优先实现“system + 单条 user 的无状态文本调用、按能力处理标准输出上限/格式、可信响应元数据”。聊天历史、多模态、外部工具和 Agent 自主执行是独立通用功能，不因至简当前需求而纳入首批。没有公开实现路径的参数继续明确不可用，不能吞掉字段假装支持。

## 2. 限制必须按层说明

| 层 | 责任与能力依据 | 禁止的推断 |
| --- | --- | --- |
| 底层模型/供应商服务 | 精确型号/部署/模式的官方元数据、公开模型 API 或授权实测 | 厂商原生 hy3 API 支持某参数，不等于 CodeBuddy 订阅出口也透传 |
| 官方 CLI/SDK/App Server | 已安装版本的公开 schema/help、稳定 SDK 元数据 | CLI 有通用旗标，不等于所有所选模型可接受任意值 |
| Mux Adapter | 参数和消息映射、进程控制、结果解析 | Adapter 尚未接入，不得写成底层模型 UNSUPPORTED |
| Mux Gateway | 请求 schema、授权、规范化、模型选择与校验 | Gateway 拒绝 response_format 不证明底层模型不能返回 JSON |
| 客户端 | 业务 Prompt、预算、字段验证和任务恢复 | 客户端目前没用 history，不意味着 Mux 需要删除通用会话能力 |

当前 `codebuddy/hy3` 的 max_tokens 400：Gateway 接受这个字段，但 CodeBuddy Adapter 目录把所有参数声明为 UNSUPPORTED，Gateway 在启动 CLI 前拒绝。该请求没有证明 hy3 拒绝了 max_tokens。CodeBuddy 2.161.0 公开 CLI help 未找到回答 Token 上限旗标；这是当前公开出口的证据缺口，不是模型不支持的证明。Qoder 1.1.64 则明确有 `--max-output-tokens`，未使用它属于 Mux 实现缺口。

## 3. 通用能力模型

复用现有 ModelDescriptor/ChatCapabilities 与 Key 过滤后的 `/api/chat/models`，增量扩充并文档化；不先引入第二套独立目录、全局 Provider Registry 或客户端专属接口。`/v1/models` 保持 OpenAI 兼容形状，扩展能力接口有独立 schema_version；旧字段在过渡期继续表达“当前有效接口能力”。

每个能力至少保存：

| 字段 | 定义 |
| --- | --- |
| effective_support | 当前调用链可用状态：SUPPORTED / UNSUPPORTED / UNKNOWN；只有完成映射和相应验证才可支持 |
| native_support | 精确模型/服务的声明；未获得则 UNKNOWN，不照搬同族网页 |
| public_interface_support | 该 CLI/SDK/App Server 已公开的能力及版本；可与 native_support 不同 |
| adapter_state | IMPLEMENTED / NOT_IMPLEMENTED / VERSION_UNSUPPORTED / UNKNOWN |
| limiting_layer / reason_code | 造成当前不可用的层与原因：gateway/adapter/public_interface/model/account/unknown |
| evidence | 来源类型、版本、观测时间；目录、公开契约、fixture、授权真实测试分别记录 |
| constraints | 范围、枚举、模式/角色组合、预算计数口径及格式保证程度；缺上限使用 unknown，不用 0 冒充 |

身份以 connection instance + Adapter + 型号/路由 ID + 协议 + 版本为边界；支持当前 adapter ID 的兼容读取，连接实例迁移沿用现有产品计划，不重复建模。配置、目录/参数声明、账号可用性与真实质量验证是独立状态。

能力维度包括：文本/图片等输入、system/developer、单次多消息、stateless history、native session resume、非流式/SSE、JSON object/JSON Schema 及保证级别、输出上限、采样、推理档位/开关、上下文界限、usage、结束原因和取消可证性。某一维未实现不得让整型号消失；无法满足本次请求才拒绝。

Codex 优先保留 model/list 的 inputModalities/defaultReasoningEffort/supportedReasoningEfforts；Qoder/CN 可通过各自公开 SDK 的 getAvailableModels 取得 context_config/thinking_config，不从内部缓存或服务私有端点补数据。CodeBuddy 从公开目录读取 ID；型号参数元数据没有稳定出口时明确 UNKNOWN，SDK 通用选项仅登记为接口候选能力。

## 4. 对外 API 与规范化

### 4.1 最小通用文本契约

首批要求 model、messages、非流式响应以及可选 stream。消息最小支持“前置 system/developer 指令 + 单条 user 文本”。多个连续前置指令可按原顺序以明确边界合并，并记录这种规范化；不能只取最后 user，不能把已有对话历史改成普通资料。system/developer 的优先级映射若无法保持应报错，不能无声降级。

history=SUPPORTED 专指客户端在一次 Chat 请求中提供完整 user/assistant 历史仍能保持语义；session_resume=SUPPORTED 专指上游会话可续接，两者独立。只支持 native resume 的 Adapter，不因此宣称能导入任意 Chat 历史。非前置 system、tool 消息及未实现的组合返回带 param/limiting_layer 的错误。首批不实现自动 flatten 全部 messages；客户端可以显式选择自己管理的单 user Prompt，但 Mux 不负责其业务优先级。

### 4.2 输出长度与可选生成参数

- 保留兼容字段 max_tokens，并规范化支持标准 max_completion_tokens；同时提交二者时报明确冲突，不静默择一。是否映射同一上游选项需核实其计数口径（可见答案/推理+答案/单模型请求/整 Agent turn），不能一律当作等价别名。
- Qoder/CN 的公开 `--max-output-tokens` 是映射候选；完成版本、值和计数口径核对后按型号开放。CodeBuddy/Codex 没有已证实的回答上限出口时保持不可控/UNKNOWN；不能用 max-turns、思考预算、费用预算或停止读取字节伪装成硬 Token 上限。
- temperature/top_p 仅在精确型号的稳定公开出口可传且有合法范围时透传；首批不为缺少此出口的 Agent 加私有参数通道。省略使用模型默认值；客户端显式传不可用参数就报错，不自动吞掉。
- 使用通用 reasoning_effort 作为候选标准字段，按该模型枚举映射 App Server effort 或 SDK 控制；不强制所有模型提供 off/none。low 不等于关闭思考。Qoder 的 thinking 模式、CodeBuddy SDK thinking 配置可以是内部映射，但不能在 Gateway 为某厂商照搬一整套私有字段。
- context_window/num_ctx 是上游能力或特定 runtime 的设置，不要求所有 Gateway 型号接受它。Mux 报告有效输入界限，不伪造/强行扩大模型窗口。

### 4.3 结构化输出

按标准 response_format 区分 text、json_object、json_schema。支持程度以 generation_constraint / upstream_validation / client_validation_required 表示；CLI 的 `--output-format json` 只规定 **CLI 事件包装**，不保证模型答案为 JSON。

CodeBuddy 的 `--json-schema` 与 Codex turn/start.outputSchema 是公开候选路径，需核实输出字段、增量/最终答案、工具禁止模式、Schema 子集、内部修复次数和最终失败行为，再开放。json_object 可映射到已证明可用的对象约束，但不能靠“请输出 JSON”广告为原生强保证。Qoder 未取得原生输出 Schema 证据时只提供文本答案；客户端自行 Prompt + 解析的能力不得误标成 Mux JSON mode。

Schema 验证是通用协议能力；Mux 不包含至简的 Evidence ID、逐字引文、POI、章节或任务实体规则。不为至简固定 JSON Schema，也不把业务材料留存用于模型自动修复。

## 5. Adapter 改造路径

| Adapter | 首批可评估的公开映射 | 首批保持边界 |
| --- | --- | --- |
| Codex 0.160.0 | 指令→thread/start 的 baseInstructions/developerInstructions；reasoning_effort→turn/start.effort；Schema→outputSchema；usage→thread/tokenUsage/updated | 两种指令字段的覆盖/追加语义必须先核实；没有已知硬输出上限不补造；Chat history 导入不借助未授权实验方法 |
| Qoder 1.1.64 | system→--system-prompt；输出→--max-output-tokens；思考/窗口依据型号元数据；公开 SDK 获取目录与 usage | 单纯添加旗标不能宣称 17 型号逐个支持；不要把全局版/CN 登录或元数据混用 |
| Qoder CN 1.1.64 | 独立官方路径/SDK，映射同类已公开控制 | 独立 14 型号及约束，不能从全局版复制某型号的默认值 |
| CodeBuddy 2.161.0 | system→--system-prompt；JSON Schema→--json-schema；结构化输入/SDK 用于显式消息与元数据；公开 SDK thinking/effort 作候选 | CLI 无输出 Token 控制证据时保持限制；SDK 文档存在不代表已安装配对版本兼容所有 17 型号；hy3 不借其他供应商端点穿透订阅 |
| HttpProvider / 将来已授权的公开本机 API | 完整 messages/标准参数按服务的型号能力映射，保留 usage 与结束信息 | 当前并无已连接这类型号，不为满足客户端虚增目录或接入账号 |

不为加两个公开旗标全面迁移至 SDK；只有目录/消息/结构化事件确实需要 SDK 时才引入其受控 sidecar，且锁版本、明确进程退出、账号隔离、公开 transport 和最小元数据白名单。不能修改供应商全局设置或默认模型来模拟 per-request 参数。

## 6. 响应、错误与执行规范

现有 NormalizedResponse 只有 text/session ID，NormalizedStream 只有文本字符串，usage/结束原因/实际路由因而无法完整传到 Gateway。改造为通用元数据响应及带类型的 delta/usage/completed/error 事件，保留文本增量兼容包装；无需复制厂商整套事件到外部。

1. 映射正常结束、长度/上下文截断、取消、拒绝、超时、失败与未知；停止读取不能冒充上游 stop。需要新增 SDK 类型/测试，不让各 UI 独立猜结束状态。
2. usage 按 input/output/reasoning/cache、来源和可信度记录，明确每请求与累积会话口径。Codex last/total、Qoder model_usage 累积值须防重计；UNKNOWN 用缺省/null或明确状态，不把 0 当真。保持 OpenAI 兼容消费，并评估旧客户端对未知用量的解析回归。
3. 对动态 Auto/档位返回实际上游 ID（若上游提供）与请求 ID；未报告就标未知。不以答案自述识别型号，不猜 Sonus/Cantus/space-bunny。路由策略不暗中换账户或计费连接。
4. 配置/协议错误带稳定 code、param、limiting_layer、retryable=false；区分 not_implemented、unsupported、unknown constraints 和 out_of_range。保留安全 request ID，不返回原始认证信息/Prompt/错误正文。429/认证/额度/网络/5xx/上游冷却独立处理。
5. 取消“请求已停止接收”和“上游已停止生成”的保证分开。共享上游重试/节流和 Mux 队列都有界，心跳不计作回答，用户没有授权时不触发真实验证。
6. 标准文本推理默认 INFERENCE_ONLY 语义、隔离 cwd、tools/MCP 关闭、配置来源受控。当前 Agent Key 权限边界保持，不能只改 kind=Provider 来绕过授权。只读沙箱不等于无工具；需要 Agent 工具时走独立明确授权的执行范围，不在普通 completion 自动升级。

## 7. 责任边界：通用性与不做事项

| LocalAiMux 负责 | 客户端（包括至简）负责 |
| --- | --- |
| 标准协议、字段和角色规范化、能力及限制层、公开上游映射 | 业务 system Prompt、资料优先级、业务 JSON Schema、Evidence 校验 |
| 连接/Key/模型/账号隔离、通用队列与请求级限制 | Job/Stage 编排、Map/Reduce、Artifact/Replay、业务重试预算 |
| 通用 usage、结束状态、错误和实际上游身份 | 用量账本、阶段预算、业务缓存与配置快照 |
| 参数不可用时明确拒绝；不暗中换型号/账户 | 选择是否省略非必要参数、采用默认值或选择其他模型 |

Mux 禁止新增 zhijian/job_id/stage/segment_id/poi 等特定业务路由或配置分支；不把至简的 64 Token 测试、关闭思考、笔记 Profile、Schema、Domain Pack、中文提示词写成 Gateway 默认值；不自动 flatten 资料来满足一个客户端；不为了至简伪造 Token 用量、输入窗口、支持状态或型号。同等 OpenAI-compatible 客户端应获得相同的规则和行为。

多轮、视觉、工具、自主 Agent 和动态路由可以作为 Mux 独立产品能力，但有自己的公开协议、授权与验收；不作为这轮客户端兼容的捆绑实施范围。继承客户端不必要参数不是“兼容性”，保留真实限制也不是“模型不行”。

## 8. 至简参数要求的精简决定与适配计划

| 要求 | 精简后决定 | 至简侧方法 |
| --- | --- | --- |
| model、system/user 业务语义、答案文本 | 必需 | 保留自包含请求；连续核心/补充指令明确组织，不丢 Evidence/身份材料 |
| JSON 可解析及业务字段/Evidence 有效 | 必需 | 继续服务端验证；原生 JSON mode/Schema 不是所有型号的准入必需字段 |
| max_tokens 每次必传/Probe 强制≤64 | **取消普遍必需性** | 输出上限改成 optional；仅支持且口径可用时发送。连通测试单 user，按能力决定限额，不发送已知拒绝字段 |
| temperature/top_p、固定采样 | **非必要，默认省略** | 当前没有必要增加 top_p；显式选定偏好才在支持型号发送，不用于“一刀切”准入 |
| thinking=false、全模型必须关闭思考 | **取消普遍必需性** | DeepSeek 等已支持路径保留；Mux 使用默认/有证据的档位，不把 low 说成 off。个别预算严格任务可主动换合适型号 |
| context_window/num_ctx 通用透传 | **取消跨接口要求** | num_ctx 仅留在 Ollama；远程窗口用于本地分包及估算，不强迫 Mux 接受 |
| 全模型必须原生 JSON Schema | **取消普遍必需性** | 能原生约束就用；否则 Prompt + 原有 JSON/业务校验，不降低落库质量门禁 |
| history/会话续接/previous_response_id | **当前不用** | Job 恢复继续依赖配置和 Artifact，不开发聊天会话系统 |
| SSE、图片、tools | **本次不要求** | 首批用现有非流式文本；视觉留在独立已验收 Provider，不把所有 Mux 型号当视觉型号 |
| usage 精确已知 / 硬 Token 上限 | 不作为全型号共同前提；需如实未知 | 未知使用保守估算、输入分包、尝试/耗时/响应大小上限；严格费用/Token 预算阶段选择有相应控制的模型，不能绕过已有冻结总预算 |

剔除的是线缆参数的一刀切要求，不是 JSON、Evidence、预算、Secret、任务身份或 POI 人审门禁。任何剩余总预算策略调整必须按至简现有冻结规则评审；此文没有执行配置更改。

至简实施位置与顺序：

| 包 | 文件/方法 | 依赖与验收 |
| --- | --- | --- |
| Z1：测试与参数协商 | providers/llm.py、ai/model_registry.py、Profile/Stage 非密钥能力配置；复用现有 option=None 的省略路径，移除 Probe 无条件 64 | 目录能力读取不调用推理；Ollama/DeepSeek 回归；旧 provider test 的默认 response_format 也要覆盖 |
| Z2：消息规范化 | services/video_support.py + providers/llm.py JSON 修复：把同次指令显式组织成前置 system + user，保持优先级与证据内容 | 依赖 Mux M2 最小角色映射；无能力就提前诊断。单 user 业务 Prompt 编译如有必要只能在至简显式、版本化启用，不由 Mux默默做 |
| Z3：错误、预算与快照 | ai/reliability.py、ai/gateway.py、预算/调用审计与缓存、Job 非密钥快照 | 配置类 400 不计入暂时故障熔断；未知 usage 不作 0；能力/规范化版本入 Cache Key/快照，主备分别协商 |
| Z4：设置与授权验收 | 设置页显示来源层、未映射/默认参数、不可用原因；随后授权串行小样与代表性视频 | PC/Mobile Browser 验收；不改历史能力 PASS/FAIL，不自动重跑或确认 POI |

## 9. Mux 实施工作包与验收

| 包 | 具体范围 | 完成门槛 |
| --- | --- | --- |
| M1：能力来源与身份 | domain、Adapter SDK、三类 Adapter 目录、Key 过滤能力接口；保留当前有效状态，增加来源层/版本/约束 | 所有 55 ID 无遗漏；Codex 7 型号元数据不丢；Qoder/CN 官方元数据能读则按型号读，失败/未配对保留未知；没有任何真实请求 |
| M2：最小通用请求 | api-types、Gateway、Adapter 指令与公开输出限制映射、response_format 候选路径 | system+user 与连续前置指令、不支持组合、参数冲突/范围都有 fixture；不会只取最后 user；不依赖业务词和至简 Schema |
| M3：响应规范 | NormalizedResponse/typed stream、usage/finish/reasoning/actual model、安全错误、工具与配置来源约束 | 非流式与 SSE 同义；累积 usage 不重计；未知/截断/失败不误报 stop 或 0；取消保证明确；普通推理不进入工具执行 |
| M4：可选公共能力 | 仅经单独选择实施 history、native resume、视觉/工具等；首批可延期 | 不能用 session resume 冒充任意 Chat history；各功能各自授权和验证，不阻塞最小文本交付 |
| M5：互操作与真实验收 | 通用测试客户端 + 官方 OpenAI SDK + 至简；先离线，后授权串行小样 | 同请求适用于至少两种客户端，无特判；逐 Adapter 代表样例，型号差异单列；真实推理后才更新 live_verified，不批量消费 55 模型 |

代码阶段按 AGENTS.md 的 crate/前端验证、CHANGELOG/交接及本机部署流程执行；应用有任务先等待，不强制重启。原设计阶段为文档-only；本次代码实施已按流程验证并部署，记录见实施验收。Windows、所有 55 型号真实生成、质量 Benchmark、会话/取消计费效果均不能由这份盘点代替。

## 10. 采纳条件

建议先实施 M1→M2→M3 与至简 Z1→Z2→Z3，最后做 Z4/M5。M4 不混入首批；CodeBuddy/Codex 输出上限尚无稳定证据的部分保持限制，而不是给至简加兼容后门。评审需要确认的是阶段范围和验收预算，不是是否给至简提供特殊 API。本设计要求一般客户端复用同一能力、参数和错误契约。
