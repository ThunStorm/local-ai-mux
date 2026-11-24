# Gateway 可靠性验收（2026-10-10）

范围：[独立计划 M0–M6](../LOCALAIMUX_GATEWAY_CAPABILITY_RELIABILITY_PLAN.md)，仅本仓库，M7 后续选做。复用目录 v2、Key/权限、现有 lane/熔断、typed events、call_records/正文开关、官方 SDK smoke；没有至简特判、真实生成、新连接、授权升级或 Key 重建。

## 兼容与门禁证据

| Golden | 本轮证据 | 结果/边界 |
| --- | --- | --- |
| G1 目录/权限 | chat_capabilities、security_policy、installed_catalog 默认 ignored | 离线 PASS；实际账号目录不自动重读 |
| G2 角色/历史 | capability_negotiation、Codex/Qoder/CN/CodeBuddy fixtures | 前置指令保序，未支持历史/developer/非文字明确拒绝 |
| G3 参数/来源 | v2 freshness/expiry、冲突/范围/未知/Schema 回归 | PASS；模型保留；原生 JSON 保证和上限未知不广告 |
| G4 规范化 | leading_instructions_v1 + 独立 outcome/finish | PASS；规范化不冒充推理或 JSON 修复 |
| G5 用量/型号 | last/request 快照、来源词表、有界型号、累计999不能覆盖本次14 | PASS；无事实来源为 null/unknown，localmux 固定键≤4096字节 |
| G6 两接口/流 | openai_compat、responses_compat、capability_negotiation + 官方 SDK | 非流/SSE的usage/length/cancel同义，失败无虚假completed |
| G7 期限/资源 | execution 虚拟时钟、stream_failures、SDK disconnect、SDK CLI/进程组 fixture | 队列25+执行10；30秒queue/5秒间隔；600整体/180空闲；未消费Body独立到期，lane释放 |
| G8 隐私/历史 | call_records、storage migration12→13、recording/redaction | store:false 不写新诊断/正文/映射/事件；旧NULL保留、字节上限8192 |
| G9 官方 SDK | scripts/sdk-smoke.sh，Python3.24.0、TypeScript7.23.0 | 各25 cases / 0 failed；目录/v2/关联、Chat/Responses/SSE、unknown、length/cancel/disconnect、400/429/503 |
| G10 GUI | Playwright + 已安装 Edge，合成 IPC，960×680/800×600/760×560/1120×760、浅深 | 扫描/验证连接、启停、主动复制合成配置、历史/旧NULL/忙碌/过期能力/Escape；控制台无错误；不模拟接入真实Provider |
| G11 升级 | 0013 fixture、默认Key跨重启测试；真实部署一致备份/校验在 HANDOFF | 离线及本机升级 PASS；schema13、Key/权限/4个连接、录制设置与48条历史完整保留 |
| G12 平台 | 本机 macOS x86_64 工具链；Unix假CLI | macOS可执行部分单列；Windows/原生ARM64/真实上游 NOT_VERIFIED |

官方 SDK 使用单独内存数据库和合成 Key，两个客户端各重启一份 fixture 服务，避免429冷却互相污染。只调用 mock/fixture，NO_PROXY固定回环，CLI fixture都是本仓库假进程。首次新增 SDK 矩阵揭示 fixture 的 Responses 误走旧文字包装，已改为 typed session events。断开初测经本机代理，代理保留连接，不作为Gateway无法检测直接断开的证据；绕过代理后通过，不修改用户代理设置。

## 测量基线

`cargo test -p localmux-gateway --test runtime_baseline -- --nocapture`：40个合成非流请求，20个store:false、20个保存元数据。此次 no-store 中位335µs、保存元数据1055µs；诊断最大630字节、SQLite已分配页增长20480字节。这是本机一次小样本，不是云端性能/Token节省或长期容量承诺；没有正文录制、无真实Provider调用。队列/Adapter分阶段和取消/锁释放另由受控时钟fixture验证。记录总耗时到终态写入前，纯云端执行、非流首字、提交后finalize时间留空；SSE字节为首终态帧前缀，不能当完整传输量。

## 发布与回滚

必跑命令：`bash scripts/verify.sh`、`cargo clippy --workspace --all-targets -- -D warnings`、`LOCALMUX_TEST_PYTHON=<installed SDK Python> bash scripts/sdk-smoke.sh`；兼容服务另做feature-enabled Clippy。最终整树 Rust120 passed / 0 failed / 2 ignored、前端7 passed、Clippy及feature-enabled harness Clippy通过；SDK各25 cases / 0 failed。并行工作树门禁及安装SHA见[HANDOFF](../HANDOFF.md)。本轮曾遇并行托盘新增测试失败，按用户要求保留、等待该任务完成后才做最终全量和本机部署，不把独立Gateway通过当作整树通过。

迁移只有0013。回滚先确认任务0，保留升级后DB/凭据一致性备份，再恢复pre-0013 DB和匹配旧程序；不可用旧SQLx程序直接打开新schema或篡改用户迁移账本。新记录先保留，不自动删除、回填或改用户开关。源码回滚不改Key/scopes、连接、路由/额度历史。进程组只清理仍拥有的孤立leader，已reap后不使用陈旧PID；逃离组的后代、Windows树与远端取消/计费继续UNKNOWN/NOT_VERIFIED。

本机交付：2026-10-10从 `codex/dev / e6e31e8 + 已保留的图标/托盘工作树` 统一编译安装重启。二进制SHA-256 `047aaf87b6776cfa4f8bd63a1e5628056de0347e99bc55dda3b195d4e59513d9` 与release/build-info一致；安装程序为x86_64，dmg文件名不是ARM64运行证据。正常退出前任务0、SQLite一致备份；schema12→13 后integrity=ok、FK=0，原Key/scopes/限制、4个连接、录制开关及48个调用ID完全保留，旧diagnostics仍NULL。API恢复部署前运行状态、health保护值600/180/30/5且任务0；原生主窗口/API/历史与录制状态可用。没有生成或清理用户记录；新诊断故障矩阵由隔离fixture验收。
