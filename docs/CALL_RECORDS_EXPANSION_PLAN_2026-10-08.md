# 调用与会话记录扩展计划

状态（2026-10-09）：原工作包已完成并追加正文录制开关与界面整理；本文 2026-10-08 的元数据边界和 97 项测试结果是当时交付记录。当前契约以 [SESSION_MODEL](architecture/SESSION_MODEL.md) 为准，最新 105 项测试、schema 12、本机部署及远程交付范围见 [HANDOFF](HANDOFF.md)。

## 目标与验收

把高级区“会话记录”扩展为可以追踪所有本机生成调用的入口：Chat、Responses、桌面试对话、模型短测试均可查；成功、失败、超时、取消、截断、过滤和异常中断均有明确状态。记录与上下文续接分开，失败记录不会使不完整回答进入后续历史。默认只保存元数据，不保存正文、请求参数、客户端 metadata、凭据或上游原始错误。`store:false` 不写调用记录或 Responses 映射。

## 当前问题

原记录仅在 Responses 完整完成后写入 conversations/responses/provider_sessions，Chat、试对话和短测试无持久记录，失败调用无法排查。会话列表固定 100 条，缺少状态、来源、耗时、用量和清理入口。既有路由审计事件与原生续接映射继续保留，不补造过去的调用。

## 工作包与实施顺序

1. **持久化与迁移**：新增 `0011` 调用元数据表，字段包括本地调用 ID、Key 所有者、不透明会话/Response ID、来源、接口、请求型号、实际型号/Adapter、流式标记、开始/结束时间、本机耗时、状态、安全错误码、结束原因及已报告 input/output/total tokens。未知数字为 null。启动恢复遗留运行中记录为 interrupted；已有连接、Key 和续接映射原样保留。分页使用稳定的自增游标，按来源/状态/型号筛选，提供单条和已完成记录清理；删除调用记录不删除续接映射。
2. **Gateway 生命周期**：共用记录封装，认证通过且 JSON 可解析后登记；未认证/超大/畸形请求不入生成历史。能力/权限/路由等后续拒绝保留失败状态。Chat 与 Responses 同时接受标准 `store`。调用开始必须成功持久化后才允许生成；HTTP 错误只提取 LocalMux 安全码。typed 元数据在非流式和流式路径直接更新记录，不从正文猜测用量或型号。取消请求、丢弃 SSE、启动失败和超时均结束记录；进程异常退出留待启动恢复。记录不改变现有超时、队列、收费/重试和权限策略。
3. **桌面入口**：试对话经现有固定 loopback 请求标记来源，仅改变审计分类，不增加权限。模型短测试复用相同记录生命周期。高级“会话记录”默认显示调用历史，附来源/接口/状态/耗时/已报告用量/错误码，支持刷新、筛选、加载更多、详情与清理确认；另保留原有续接会话/路由事件入口。切换页面重新加载，IPC 失败可见，不静默空白。
4. **回归验证**：隔离 SQLite + Mock/故障 Adapter 覆盖两接口、流/非流、store=false、鉴权、能力拒绝、超时、部分输出/过滤/取消、断流、丢弃客户端、来源分类、未知/已报告用量、稳定分页、筛选、清理、迁移重开与正文/凭据不落库。保持原 Responses 最新完成 ID/Key 隔离及原生续接测试。执行存储/核心/Gateway 测试、前端类型/测试、全仓 verify、Clippy、官方 Python/TypeScript SDK smoke。
5. **文档与本机交付**：同步 SESSION_MODEL、DATABASE、OBSERVABILITY、OPENAI_COMPAT、STREAMING、UX_SPEC、CHANGELOG Unreleased 和 HANDOFF。部署前确认任务为 0、数据库完整并备份；从当前工作树运行部署脚本，正常退出安装重启，核对迁移、进程、构建哈希和原生记录页。真实 Provider 生成/连接不作为离线验收替代，不自动触发；Windows 原生安装记录为未验证。

## 边界与明确取舍

- 本轮只实现元数据记录。正文持久化/导出需独立隐私设计，不添加一个无法兑现的保存开关。
- 不抓取其他应用的历史，不回填旧 Chat/短测试。旧 Responses 会话仍可单独查看。
- Chat 没有可靠服务端会话标识，每次请求独立记录；不按相同 Key/型号猜测归组。Responses 通过现有 conversation_id 关联各轮；非完整响应只可查询，不进入 responses 成功映射。
- `store:false` 覆盖成功与失败路径；没有不保存选项的 UI 短测试只记元数据。
- 客户端来源标签只用于展示，可被客户端模仿，不作为认证/权限证据。记录中的实际目标采用 Adapter/Gateway 已解析结果，拒绝调用不冒称已调用上游。
- 清理只删除终态调用记录，运行中记录不可删除，Responses 续接映射继续存在并由原入口管理；未实施自动保留期。
- 本机停止接收记为 cancelled，不承诺上游取消或停止计费。正常 EOF 缺少完成事件记为 interrupted，重启遗留记录也记 interrupted。

## 完成记录

工作包 1–5 已完成。实现、离线/SDK/浏览器验证、文档同步和本机安装重启已闭环；下面保留真实 Provider 与 Windows 原生未验证边界。

- `0011` 与独立记录层、Chat 标准 store、两协议生命周期、短测试来源、桌面 IPC 与页面已实现。Responses 按已有映射关联各轮和可定位到同会话的续接拒绝；失败映射不能冒充可续接成功。来源字段不参与权限。
- 最终 `bash scripts/verify.sh`：Rust 97 passed / 0 failed / 2 ignored（需显式 CLI 路径的目录检查）；前端 7 passed / 0 failed，格式、类型和构建通过。全仓 `cargo clippy --workspace --all-targets -- -D warnings` 通过。新增 7 个 Gateway 集成测试、1 个记录层单元测试和 2 个存储测试覆盖记录/隐私/取消/迁移/分页/清理；原 Responses 链回归另核对 store=false 不写路由事件与完整/拒绝记录按会话关联。
- 官方 Python OpenAI 3.24.0 和 TypeScript OpenAI 7.23.0 对最终源码的隔离 Mock Gateway 通过目录、Chat/Responses、流式、能力协商/unknown 元数据及两接口 store=false smoke；生产 Gateway 未添加 Mock。
- Browser 插件未配置。使用已有 Playwright + 本机 Microsoft Edge 的隔离 headless 实例，在 `http://127.0.0.1:1421/` 注入合成 IPC 元数据做页面验证：标题/非空/无错误层、56 条分页、来源/状态过滤、按会话查看、详情、取消删除、单条/全部清理、删除过程中切换筛选、运行中禁止删除、读取失败均通过，控制台无错误。960×680 与 760×560 截图无水平溢出，深色在外观过渡结束后背景/文字对比正常：`/tmp/localmux-call-records-ui-960.png`、`/tmp/localmux-call-records-ui-760.png`、`/tmp/localmux-call-records-ui-dark.png`。这是浏览器 fixture 验证，不替代原生 IPC。
- 首轮故障已修复/解释：误用测试 Adapter 能力构造名、Responses fixture 未覆写 typed session stream、前端 ES target 不支持 Array.at 与缺少明确可访问标签已修正。虚拟时钟驱动的超时/队列测试隔离 SQLite worker 调度（store=false，5 秒间隔使用真实时钟），持久化另由真实时钟集成测试覆盖；一次 Apple 链接器 SIGSEGV 以单任务编译重试通过，不改变应用依赖。浏览器 fixture 补齐 Tauri 事件清理接口后控制台为零错误。
- 相关相对文档链接与 `git diff --check` 通过。临时 SDK venv、浏览器截图/脚本和日志留在 `/tmp`，没有加入仓库。

收尾补强：已知终态在发送完成帧前落库，SDK 收到 completed/[DONE] 后立即停止读取也不会留下运行中记录；SQLite 拒绝终态写入的故障测试确认不输出成功帧，补写不能改回成功。请求型号误填本机 Key 或常见凭据形态时替换为无效型号标记，不保存原值。删除后的刷新读取当前筛选快照，延迟删除期间切换筛选的 browser 回归通过。

## 本机交付与未验证边界

- 部署前 active_tasks=0、数据库 integrity=ok，备份 `/Users/williamli/Library/Application Support/LocalMux/backups/localmux-pre-call-records-20261008-181240.db`。最终 `bash scripts/deploy-local-macos.sh` 从 codex/dev / 248f675 + 工作树构建安装并正常重启 `~/Applications/LocalAiMux.app`；安装主程序、release 主程序和 build-info SHA-256 一致：`7e0b80a2c0fdf4e388d49f68d6848fefe67e2aa74512527e47064ad23ce6568f`。此前构建后的收尾修改均已纳入最终重新打包，不以旧安装包代替最终源码。前端构建资源为 `index-DgKjYnUW.js` / `index-nbf0Kd6e.css`。
- 安装版迁移 11、完整性 ok、原四个连接保留；默认 Key ID/哈希/完整权限记录与部署前备份一致。原生正常启动后 API 已恢复运行，活动任务 0。现有工作树中的超时/队列/Adapter 修复共同参与全仓构建与验证，本任务没有回滚这些改动；不扩大其真实 Provider 验收结论。
- 最终安装版用当前 Key 发送两条认证通过、生成前即拒绝的合成请求：Chat max_tokens=0 与 Responses 非法角色输入，均 400 LMX_INVALID_REQUEST，记录分属 playground/chat 与 api/responses。原生 AX 与截图确认两类记录、来源筛选到单条、型号无匹配空态、恢复筛选、详情及删除确认/取消；界面非空、无可见错误层，深色稳定状态可读。未读取原生 WebView 控制台；零控制台错误结论仅属于 browser fixture。两条临时记录已按精确 ID 清理，没有删除用户历史/续接映射，最终记录表为空符合升级前无记录且不回填的语义。
- 未触发 Provider 新连接/登录或真实生成。成功/流中断/超时/截断/取消等生成终态由离线 Adapter 和 SDK 验证；真实上游取消/计费不作承诺。Windows 原生安装、ARM64 原生二进制、真实正文保存/自动保留期仍未验收或不在本轮范围；实际 Mac 主程序为 x86_64，dmg 文件名不作为 ARM64 证据。
- 未提交、推送或发布 Release，本轮工作保留为本地改动；相关规范、CHANGELOG Unreleased 与 HANDOFF 已同步。

## 2026-10-09 后续范围修订

用户明确要求完整对话及录制开关；先前“本轮仅元数据/无正文开关”是 2026-10-08 交付的历史边界。现在通过 0012 的默认关闭、持久开关和独立正文表增量实现，关闭停止在途录制、store:false 优先、凭据脱敏、旧记录不回填；具体当前契约见 [SESSION_MODEL](architecture/SESSION_MODEL.md#对话正文录制2026-10-09)，实际验证与部署见 HANDOFF 最新条目。
