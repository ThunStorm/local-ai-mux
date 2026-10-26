# LocalAiMux 当前交接

## 当前工作基线

v0.2.0封版：CLI空闲上限30–1200秒，Gateway入站总期1200秒；默认180秒与已保存值保留。统一LocalAiMux品牌和workspace版本，补充版本/文档门禁、TODO与运行说明。两个已修复隐性问题（设置未知结果误显旧值、HTTP非流式输出无界）见[检查记录](releases/V0_2_0_BUG_AUDIT.md)。完整内容和验证见[封版检查](releases/V0_2_0_FREEZE.md)。

- 原开发基线：codex/dev / 84d09e2；当前提交/标签以Git为准。
- 当前本机CLI已保存600秒（保留用户原值），支持30–1200；Gateway总期1200。0.2.0已安装重启、API恢复运行。
- 不自动真实调用Provider、不新增连接、不重建Key、不改历史数据；Schema保持13。
- 已实现的会话导出、紧凑工具区、额度同源、缓存诊断与CLI超时层归因保留。
- 真实质量/远端取消/签名发行/Windows与ARM64边界见[TODO](TODO.md)。

## 验证与部署

2026-10-26：Rust142/前端12、类型/构建、全仓及harness Clippy、Python/TS SDK各25均通过；三尺寸中英浅深、未知保存/1200/1201、筛选导出与独立文件读回通过。原生0.2.0与1200保存读取已验，恢复600；health版本/总期正确、任务0。备份 `/Users/williamli/Library/Application Support/LocalMux/backups/localmux-pre-v020-20261026-104852.db`；安装/release/build-info SHA256 `c991a531624b4071291e1b6cb036cf46e07b513e8acc42be5f02b4294ac2251a`一致，PID43913存活，实际x86_64。原257条调用/正文/映射、Key/权限、连接和settings保持，仅额度快照刷新，integrity ok/FK0、无迁移/真实生成。源码manifest一致。完整范围见封版记录，远端CI绑定本次推送候选，v0.2.0标签只在检查后落定。

## 下一步

按TODO优先级选择一个独立工作包；不要把旧规划或历史待办自动当作已授权实施。

## 历史记录

2026-10-22及以前的提交、部署、数据库备份与业务验收记录完整保留于[历史交接](history/HANDOFF_THROUGH_2026-10-22.md)，其中日期/PID/版本/计时参数均为当时现场，不代表当前运行状态。
