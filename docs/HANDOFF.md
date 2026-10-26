# LocalAiMux 当前交接

## 封版后CI维护

2026-10-26：用户所报Windows17分6秒失败为dec15ee的临时DB删除OS32共享冲突；1d0f031虽通过，同实现仍有清理偶发性。已将六个文件型存储测试隔离到tempdir、重开前释放旧池、关闭后仅对Windows OS32/33限时清理重试，并新增真实删除锁Windows回归。本机storage13/13与全仓Clippy通过；远端结果以维护提交对应CI为准；Windows存储回归已前移，完整workspace检查保留。仅cfg(test)/开发依赖/文档变化，生产数据路径与API行为不变，不重启服务、不移动v0.2.0标签。详情见[CI维护](development/CI.md#封版后windows存储测试维护2026-10-26)。

## 当前工作基线

v0.2.0封版：CLI空闲上限30–1200秒，Gateway入站总期1200秒；默认180秒与已保存值保留。统一LocalAiMux品牌和workspace版本，补充版本/文档门禁、TODO与运行说明。本轮修复的问题（设置未知结果误显旧值、HTTP非流式输出无界、CI组件准备冲突、Windows测试导入及实时测试不稳定）见[检查记录](releases/V0_2_0_BUG_AUDIT.md)。完整内容和验证见[封版检查](releases/V0_2_0_FREEZE.md)。

- 原开发基线：codex/dev / 84d09e2；当前提交/标签以Git为准。
- 当前本机CLI已保存600秒（保留用户原值），支持30–1200；Gateway总期1200。0.2.0已安装重启、API恢复运行。
- 不自动真实调用Provider、不新增连接、不重建Key、不改历史数据；Schema保持13。
- 已实现的会话导出、紧凑工具区、额度同源、缓存诊断与CLI超时层归因保留。
- 真实质量/远端取消/签名发行/Windows与ARM64边界见[TODO](TODO.md)。

## 验证与部署

2026-10-26：Rust142/前端12、类型/构建、全仓及harness Clippy、Python/TS SDK各25均通过；三尺寸中英浅深、未知保存/1200/1201、筛选导出与独立文件读回通过。原生0.2.0与1200保存读取已验，恢复600；health版本/总期正确、任务0。备份 `/Users/williamli/Library/Application Support/LocalMux/backups/localmux-pre-v020-20261026-104852.db`；安装/release/build-info SHA256 `bf410a01cf7ab005b48702e54f8ae30555d5e4ae05c37eda59b68cde69a63873`一致，PID99856于2026-10-26T14:41:40.662762+08:00核验存活，实际x86_64。原257条调用/正文/映射、Key/权限、连接和settings保持，仅额度快照刷新，integrity ok/FK0、无迁移/真实生成。源码manifest一致。完整范围见封版记录，远端CI绑定本次推送候选，v0.2.0标签只在检查后落定。

## 下一步

按TODO优先级选择一个独立工作包；不要把旧规划或历史待办自动当作已授权实施。

## 历史记录

2026-10-22及以前的提交、部署、数据库备份与业务验收记录完整保留于[历史交接](history/HANDOFF_THROUGH_2026-10-22.md)，其中日期/PID/版本/计时参数均为当时现场，不代表当前运行状态。

远端CI收口：首候选852037e的sdk-compat在Rustup组件补装阶段失败，现对齐全部工作流组件准备，并固定Python SDK3.24.0基线；仅CI/测试声明/文档变化，已部署应用代码与二进制保持。最终CI与v0.2.0标签绑定修复后提交。

Windows补验：旧候选852037e的可靠性测试模块无条件导入Unix专用StreamExt导致Clippy失败，已加cfg(unix)；生产代码未变，继续等待修复后提交的完整三项CI。

时序补验：f22daf4的macOS CLI事件重置测试受真实Shell调度影响误触发250ms超时，现统一有界事件读取并用虚拟时钟/内存管道精确回归，真实owned进程测试保留。生产计时语义不变，最终仍须三项CI通过。

最终实现dec15ee已复部署（clean来源），API恢复、257条历史与设置保持；后续仅封版证据文档提交，已装二进制/实现manifest一致。最后复部署备份`/Users/williamli/Library/Application Support/LocalMux/backups/localmux-pre-v020-final-20261026-143521.db`。最终CI绑定v0.2.0标签对应提交，注释标签保存检查链接与本机摘要；不创建GitHub Release。
