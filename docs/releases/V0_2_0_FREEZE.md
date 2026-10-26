# LocalAiMux v0.2.0 封版检查

日期：2026-10-26；起点 codex/dev / 84d09e2。用户授权：调用限时最高1200秒、部署重启、清理代码、统一品牌、全面更新文档与TODO、检查并推送Git。封版是本机开发版本基线，不等同签名正式发行或全部上游/平台验收。

## 工作包与验收

1. CLI空闲配置30–1200秒；Gateway入站总期1200秒，保留已保存值和180秒默认值，核对其他独立计时层。验证边界、保存/重载、超过旧600秒的Chat/Responses流/非流、总期及不消费SSE资源回收、实际超时归因。
2. 产品版本0.2.0统一到workspace Cargo、前端package、Tauri与公开客户端元数据；产品名LocalAiMux，移除旧中文工作名。数据目录LocalMux、app.localmux.desktop、crate名、localmux扩展/API及迁移账本保持兼容；历史文件路径不伪改。
3. 清理重复限值、过时当前文档和散落版本，保留兼容接口与历史证据；加入版本/品牌/文档链接门禁，整理[TODO](../TODO.md)，明确完成、后续和外部验收。
4. 当前根说明、支持矩阵、构建/CI、API/Streaming、Adapter/存储/安全、CHANGELOG/交接对齐；原计划/旧验收保留日期并标明历史性质，不能冒充当前待办。
5. 全仓fmt/test/Clippy、前端类型/测试/构建、隔离官方SDK、导出读回、桌面页面与限时保存验证。任务0、数据库一致备份与完整性通过后部署，核对版本/哈希/PID/API及旧数据/权限。
6. 审阅diff、凭据字面量、Git目标与工作树，提交/推送codex/dev，标记v0.2.0 Git标签；不创建GitHub Release。记录CI和Windows/ARM64等实际验证状态。

## 隐性问题检查

[专项记录](V0_2_0_BUG_AUDIT.md)包含五个已修复问题、复现路径、回归入口和剩余边界；产品说明统一使用中文。

## 验证记录

| 检查 | 本轮结果 |
|---|---|
| 版本/品牌/文档门禁 | 19个Rust包、package/Tauri0.2.0、中文产品入口、108份Markdown本地链接通过；旧中文工作名无残留 |
| Rust全仓 | 142 passed / 0 failed / 2 ignored；两个忽略项需用户显式提供真实CLI路径，不在本轮自动运行 |
| 前端 | 类型检查、12项测试、Vite构建通过 |
| Clippy | workspace/all-targets及compat-harness两套-D warnings通过 |
| 官方SDK | Python OpenAI3.24.0 / TypeScript7.23.0各25 cases / 0 failed；只用独立合成服务 |
| 问题回归 | 设置未知结果保护、HTTP有界读取/超大非流结果、两协议流/非流901秒合成长等待、总期/背压/取消清理通过 |
| 导出读回 | 6格式、36005字符中文/Emoji/CRLF回答、XLSX两段重组、CSV安全前缀、无公式/HTML转义与品牌通过 |
| 渲染与交互 | Browser插件未提供，Edge/Playwright；960×680浅中、760×560深中、800×600浅英，1200保存/重载与未知返回、1201拒绝、6格式/时间/正文筛选、取消/错误/首屏记录通过；无相关console或溢出 |
| 原生安装版 | 0.2.0界面、1200保存/读取/DB持久值、1201禁用通过；恢复原600秒，API恢复运行、health service/version与1200总期一致 |

## 本机部署证据

- 最终实现部署源：codex/dev / dec15ee（clean）；后续只有封版证据文档收敛；`bash scripts/deploy-local-macos.sh`正常编译/安装/退出旧版/重启完成，未强杀运行任务。
- 部署前：20261026-104852（北京时间），active_tasks=0/in_progress=0，Schema13，integrity=ok/FK0；一致备份 `/Users/williamli/Library/Application Support/LocalMux/backups/localmux-pre-v020-20261026-104852.db`，权限0600。
- 安装：`~/Applications/LocalAiMux.app`，实际Mach-O x86_64；安装/release/build-info SHA256 `bf410a01cf7ab005b48702e54f8ae30555d5e4ae05c37eda59b68cde69a63873`一致。程序PID 99856，2026-10-26T14:41:40.662762+08:00核验存活。
- 数据：原257条调用、全部正文/diagnostics/Responses映射、Key/权限、4连接及settings逐表逐行不变；只发生正常quota_snapshots/quota_buckets刷新，integrity=ok/FK0。CLI1200验收后恢复原600，其他保护值180/并发1/队列30/间隔5保持；无新连接、真实生成或迁移。
- 本地证据：`dist/localmux/v0.2.0-evidence/`（Git忽略目录）保存前后检查、源码manifest、自动日志与读回/Browser结果；实际用户正文/凭据未纳入仓库或远程上传。旧SDK临时环境缺包，已在新的独立venv重装同版本后完成验证，没有改生产Python环境。

## 远程与封版规则

工作分支为codex/dev，远程为origin；先提交并推送候选，再检查绑定该提交的CI，确认后推送v0.2.0注释Git标签。最终远程状态以标签对应提交的GitHub Checks为准，不用旧提交的CI替代。Git标签不创建GitHub Release。

## 明确未验项目

Windows/ARM64原生安装、Windows系统凭据库/进程树、发行签名/公证/自动更新、精确Provider真实质量与远端取消/计费，本轮没有新增实际证据；仍列[TODO](../TODO.md)。本轮真实Provider生成0次，20分钟范围由代码/虚拟时钟及原生配置证明，不虚构真实20分钟模型任务。

远端补验：初候选852037e的Ubuntu SDK准备阶段发生Rustup cargo-fmt组件冲突，已修复工作流显式组件准备并固定Python SDK基线；应用源码/已部署二进制无变化。封版标签须等待修复后提交CI，不使用初候选失败结果或旧版绿色状态替代。

Windows补验：初候选852037e还在-D warnings发现Unix测试专用StreamExt无条件导入，现用cfg(unix)准确限定导入；没有绕过Windows检查或改变生产行为。

时序补验：macOS在高负载下暴露250ms真实Shell测试余量不足，已改为共享事件读取函数+虚拟时钟精确验证空闲重置，并保留真实进程清理测试；最终标签使用再次完整检查后的提交。

## 最终源码与复部署收口

最终实现提交dec15ee已修复实时Shell回归的时序依赖；同一实现已重新安装并恢复API。后续封版证据提交仅更新文档，source-manifest核对实现与已安装二进制一致，不因此重复改变运行进程。最初备份与后续备份均保留；最后复部署前任务0、一致备份`/Users/williamli/Library/Application Support/LocalMux/backups/localmux-pre-v020-final-20261026-143521.db`。截至2026-10-26T14:41:40.662762+08:00，257条调用/正文/映射/Key/连接/settings保持，仅额度快照刷新，Schema13、integrity ok/FK0、API任务0，总期1200、CLI原值600。原生1200保存/1201拒绝在同一版UI上完成，最终重启再次核对配置600保留。

远端CI只以最终封版标签对应提交为准；macOS、Windows、sdk-compat三项均须成功，标签注释记录最终检查链接及本机校验摘要。Security仅在PR触发，本轮push被跳过，不冒称安全审计通过。旧失败候选852037e/f22daf4保留可追溯，不作最终结论。
