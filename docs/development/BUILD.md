# LocalAiMux 构建与本机部署

适用 v0.3.0。需要Rust1.98.1、Node22.18+/24、pnpm11.19.0与Tauri2系统依赖。Node版本须满足TypeScript原生测试加载及Vite要求；不要使用系统旧Node来证明验证通过。

## 开发与检查

```sh
pnpm install --frozen-lockfile
pnpm dev
bash scripts/verify.sh
cargo clippy --workspace --all-targets -- -D warnings
bash scripts/sdk-smoke.sh
pnpm tauri build
```

Windows使用`pwsh scripts/verify.ps1`。SDK脚本只运行回环合成服务，需官方Python SDK，可通过LOCALMUX_TEST_PYTHON指定环境。版本/品牌/文档检查已接入verify与CI；Cargo包继承workspace版本，前端package/Tauri须一致。Browser合成IPC不代替原生保存窗口或真实Provider验收。

## macOS安装与核对

`bash scripts/deploy-local-macos.sh`从当前工作树打包，安装`~/Applications/LocalAiMux.app`，保留旧数据并处理旧LocalMux.app安装入口，正常退出旧程序后重启；若旧程序因任务保护未退出则取消安装，不强杀。来源、分支、提交、工作树与主程序SHA256记录于dist/localmux/build-info.txt。Bundle identifier `app.localmux.desktop`及`~/Library/Application Support/LocalMux`数据目录保持兼容。

部署闭环：

1. 确认API active_tasks与DB in_progress均0，SQLite逻辑备份（0600）、integrity/FK检查，记录Key/权限/连接/历史与设置摘要。
2. 执行部署脚本，核对安装/release/build-info主程序SHA256、实际Mach-O架构和PID存活；不能用DMG文件名推定架构。
3. 原生界面确认当前版本、页面与CLI30–1200秒设置；可临时保存1200验证后恢复原值，记录真实变化，不改其他配置。
4. 恢复部署前API运行状态，核对health版本/总期1200及当前空闲值、任务0、数据/权限保持。

默认API仍需显式启动。CLI空闲默认180并保留已有设置，其他层独立限制见[Streaming](../api/STREAMING.md)。本轮Schema13无迁移。默认本机API Key在Unix使用0700目录/0600文件，Windows使用已有OS凭据存储实现（原生待验）；只有旧内存模式首次升级或用户主动重建才换Key，权限独立保存。MiMo第三方凭据仍为进程内存，详见[安全](../architecture/SECURITY.md)。

## 原生材质、图标与平台边界

macOS26+类可用时使用公开NSGlassEffectView及WKWebView.underPageBackgroundColor，不启用私有接口；旧系统/浏览器保留CSS或不透明回退。系统辅助功能偏好通过主线程读取。

应用图标源为apps/desktop/public/logo-concepts/mountain-wave-2026-10-09/04-wave-sun-negative.png，旧SVG位于docs/product/design/liquid-glass-chat/icons/localmux-2026-10-05.svg；软件图标是原应用/CLI资产，来源哈希保留在同设计目录icon-sources.json。原生成样稿仅作历史。菜单栏使用独立透明template PNG；API运行时白浪/红日，停止时恢复系统模板，状态变化时切换而非持续重绘。

本机macOS结果不替代Windows/ARM64安装或系统凭据库实机。签名、公证、可信HTTPS更新源与安装/回滚演练见[TODO](../TODO.md)。Git封版标签不自动创建公开Release；当前验证和部署结果见[封版记录](../releases/V0_2_0_FREEZE.md)。

## 自定义数据目录

偏好设置→数据与日志保存目录后，下次正常启动冷迁移。部署/备份脚本不可再假设主库固定在默认路径：先读取固定引导目录`storage.json`的`data_dir`（null/文件不存在时沿用默认），核对`pending_data_dir`，存在排期时记录源与目标，按正常退出/启动执行。默认本机Key始终在引导目录，不与主库路径联动。不要在API运行中手动复制数据库或改指针。失败恢复与安全边界见[数据治理](../architecture/DATA_LIFECYCLE.md)。
