# 空间、图标与试对话实施记录

日期：2026-10-04。用户已要求完成设计后继续开发、打包、重部署和重启。本轮沿用 [设计边界](LIQUID_GLASS_CHAT_DESIGN_2026-10-03.md)，不覆盖已有未提交改动。

验收条件：全局界面空间与图标统一；试对话经当前 Key 的本机 Gateway 发送，模型/参数按实际能力启用，停止可达且不泄露正文/凭据；离线行为测试、原生窗口检查、当前树打包安装重启及哈希验证完成；真实上游与未验证平台如实另列。

## 设计补充

- 全局侧栏外宽 204 px、浮动表面 184 px，内容留白 24 px；最小窗口收为 60 px 图标栏。主内容不再用大空面板，统计/连接/操作各自形成稳定区段。
- 对话正文宽度上限 740 px，消息两侧保留 16–24 px；宽窗参数面板 248 px，默认 960 窗口先收起，按需展开。小窗面板避开底部编辑器，关闭/Escape 返回焦点；发送/停止不因参数面板被覆盖。
- 应用行用 40 px 真图标、名称/版本/连接状态和就近操作形成扫描顺序，路径与次要诊断收进展开区。已保存连接不能用绿色“已验证”标识。
- LocalAiMux 图标为银白圆角表面与蓝色三路汇合符号，使用 ImageGen 生成并通过 Tauri icon 工具产出各平台尺寸；在侧栏、窗口、Dock 和安装包使用同一源资产。
- 用户后续明确要求应用图标结合 AI/Mux，并绘制各 Agent 图标。最终采用智能菱形核心＋三路输入/一路输出图标，替换先前三路 Y 形方案；绘制 Codex、Qoder、Qoder CN、MiMo、CodeBuddy Code、WorkBuddy、CodeBuddy CN 共 7 个独立图标，统一银白表面、几何符号与尺寸。它们是风格化原创插画，不声称是官方厂商标志，身份/登录/路由保持独立。原始图、提示词与来源留在 `design/liquid-glass-chat/icons/` 和 `icon-sources.json`；早期提取图标仅作历史来源记录。

## 实施边界与顺序

1. 完成空间/图标设计与资产；公开 AppKit `NSGlassEffectView` 技术验证，嵌入现有 Tauri WebView。无法使用时提供兼容表面并记录；不调用私有 API。
2. 增加保守模型能力描述、参数校验与端到端透传；CLI 明确单轮/不支持采样参数，未知参数能力禁用。MiMo Code 可通过已安装官方 SDK 定义的公开模型元数据补充能力，读取只发生在用户连接时。
3. 试对话 IPC 固定请求目标、Key 留 Rust、单在途请求、Channel 增量、超时和取消、请求归属；正文仅内存，UI 纯文本。
4. 完成前端和 Rust 行为验证、SDK smoke、原生尺寸/外观/图标 QA；最后在空闲时修复遗留进程识别后执行正式部署，禁止强杀活跃任务。

## 实际结果

代码及离线验收已完成，原生安装/运行结果继续在下方追加。上一轮“设计 20/20”只证明原型，不作为本轮产品测试证据。

### 已验证的事实

- 全仓 `bash scripts/verify.sh` 通过：Rust 工作区 63 项、前端状态/配置 6 项，类型检查和 Vite 构建通过。全仓 Clippy `--all-targets -- -D warnings` 通过；原生材质最后的挂载谓词变更另通过桌面 Clippy。日志见 `design/liquid-glass-chat/implementation/verify.log`、`clippy.log`。
- 桌面 Rust 8 项涵盖实际 Mock Gateway 的 JSON/SSE 消费、分片 UTF-8/CRLF/多行 data、缺少 DONE 的异常断流、停止释放服务器流、跨窗口/过期请求拒绝、提前停止与单在途。HTTP fixture 实际断言完整消息与 f64 参数透传、未启用字段省略、范围拒绝；Gateway 集成测试验证权限过滤和撤销。
- 正式 React 源码的隔离 UI 验收最终 16/16，0 JS 错误：模型/参数、多轮/单轮切换、上下文只读归档、导航保留草稿、失败/停止、无隐式授权/连接、纯文本、1120×760/960×680/800×600/760×560、浅/深及不透明回退。环境是 Vite `127.0.0.1:18418`＋离线 Tauri IPC fixture，**不证明 native IPC 或真实上游已验收**。结果/脚本/截图在 `implementation/ui-qa.json`、`verify-ui.cjs` 和 `ui-*.png`。
- Browser 插件/skill 未提供；当前 `file://` 原型的浏览器控制被协议策略拒绝，没有尝试绕过该文件页面限制。正式 React HTTP 开发服务由隔离 Playwright 验证。首次 fixture 错写 scanned_roots 导致页面崩溃，修正夹具后 13/14；随后发现实际偏好菜单被工作区覆盖，修复侧栏/工作区层叠上下文并复跑 14/14。窄屏定位从隐藏文字改为真实 summary，未使用 force click。
- Python、TypeScript 官方 SDK 对独立 Mock Gateway 的模型/Chat/Responses/非流式/流式 smoke 通过；Python SDK 仅装入临时 venv，未改项目依赖/真实账户。最后的源码重建后再次运行两套 smoke 通过。
- 原图与最终 128 px 图标已视觉检查：LocalAiMux 为 AI 核心与多路汇聚；七个软件各用不同几何/颜色，Qoder/Qoder CN 与 CodeBuddy/CodeBuddy CN 明确区分。不声称生成图是官方标志。
- 视觉对照检查布局、材质、字体、图标、文案、响应式六项。沿用玻璃控制层/稳定正文层；按用户修订换新图标，采用规范 22/13 px 而非生成图夸大字号；未知参数禁用且有解释；请求预览折叠；移出列表收进诊断展开区以缩减行高；原生标题栏不由 WebView 绘制。深色截图停用瞬时 CSS 动画以捕捉稳定状态。

### 本机部署准备与剩余边界

2026-10-05 部署前，本机数据库 `integrity_check=ok`、保存连接 2；备份为 `~/Library/Application Support/LocalMux/backups/localmux-pre-ui-chat-20261005-000459.db`。上一轮遗留旧 `LocalMux.app` 通过自身应用菜单正常退出，进程检查确认移除；没有强杀。当前 API 原处于停止状态，没有推理任务。首轮安装重启成功后，原生验收发现 960 默认侧栏过早收起及 760 旧断点冲突，已改为 880 自动阈值、支持手动展开，取消不必要的搜索栏换行/错位子网格；回归增加两项检查至 16/16。问题截图 `native-first-build.jpg`、`native-apps-760-before-fix.jpg` 保留，不把旧安装版 hash 当作最后交付证据。

真实 Codex/Qoder/MiMo 等推理、采样的真实效果、远端取消/计费、macOS 辅助功能设置实际切换与 Windows 原生验收尚未通过；不以这些离线证据代替。默认 Key 仍最小权限，不自动轮换为 Agent 授权，须用户在软件中明确授权并发送。

### 最后安装版：2026-10-05

`bash scripts/deploy-local-macos.sh` 最后成功完成当前树 release（2m26s）、`.app/.dmg` 打包、安装和重启。安装路径 `~/Applications/LocalAiMux.app`，来源 `codex/dev` / `c3665a3` / modified；最终二进制 SHA-256 `0bdc432210f28eace097d38abd6469dbb4943e9cef3467fe1209c7b61d94713e` 与构建记录完全一致；进程存活；安装图标与 `src-tauri/icons/icon.icns` 完全一致。数据库完整性 `ok`、保存连接 2。此前两轮 build hash 不作为最终版本证据。

最终原生证据 [native-qa.json](design/liquid-glass-chat/implementation/native-qa.json)。已用 CUA 核对真实安装版而非原型：960×680、800×600、760×560 的导航/列表、三行可识别性、独立图标、搜索、参数抽屉/编辑器分层和 Escape 焦点；应用外观浅/系统和本地减少透明度切换有效。Native API 启停：health `ok`、活跃任务 0、无 Key 的模型目录 401；当前默认 Key 无 Agent 权限，试对话显示没有可调用型号且禁用发送，没有自动授权或调用真实模型。最终恢复默认尺寸、跟随系统、减少透明度关闭、API 停止。

截图格式由 CUA 返回为 JPEG，已用实际格式命名并核对像素尺寸：

- [960 应用列表](design/liquid-glass-chat/implementation/native-final-apps-960-light.jpg)、[800 应用列表](design/liquid-glass-chat/implementation/native-final-apps-800-light.jpg)、[760 应用列表](design/liquid-glass-chat/implementation/native-final-apps-760-light.jpg)。
- [760 参数/编辑器](design/liquid-glass-chat/implementation/native-final-chat-760-light.jpg)、[960 不透明回退](design/liquid-glass-chat/implementation/native-final-chat-960-opaque.jpg)、[960 系统外观](design/liquid-glass-chat/implementation/native-final-chat-960-system.jpg)。

开发/本机安装完成不等于全部外部验收完成；未验证项保持上节所列状态。未提交、推送或发布 Release。
