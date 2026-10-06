# 图标修订与验收

用户本轮要求：再次更换 LocalAiMux 图标；应用内图标使用原应用资源、统一展示风格、不随机生成。

验收条件：主图标有确定性矢量源；软件图标可追溯到各自已安装应用/CLI 的资源；不改颜色/几何/身份，只统一尺寸与容器；无原图时明确回退，不借用其他产品；类型/测试/渲染/图标哈希检查和当前树打包安装重启完成，实际结果留档。

## 改动

- 主图标源为 `apps/desktop/public/localmux.svg`：AI 智能核心＋三条输入/一条输出的蓝色汇流结构，银白圆角底。全部路径/颜色固定；用 Tauri CLI 输出 PNG/ICNS/ICO，不调用图像生成模型。WebView 小标使用 128 px 导出图。
- Codex 从已安装 Codex 资源 `icon-codex-light.png` 获取；Qoder/Qoder CN/MiMo/WorkBuddy 从各自 `.app/Contents/Resources/icon.icns` 获取；CodeBuddy Code 从它自己的 `@tencent-ai/codebuddy-code/vendor/assets/icon.png` 获取。仅用 sips 导出/缩放，保留原图内容。
- 软件图片统一 40/22 px、3/1 px 留白、10/6 px 圆角与中性表面/细边框；名称与连接身份保持独立，不重绘厂商标志。
- CodeBuddy CN 桌面当前未安装，没有原图时使用带名称的中性 CLI 回退；不会用 CodeBuddy Code/WorkBuddy 的图标冒充。
- 上轮生成插画只留在历史设计目录，不再用于生产。来源、源文件和导出文件的 SHA-256 在 [icon-sources.json](design/liquid-glass-chat/icon-sources.json)。

## 实际验证

本轮 `pnpm typecheck && pnpm test` 通过（6 项），正式 React 离线 UI 回归 16/16，0 JS 错误；6 个原始资源的源/导出 SHA-256 与重跑 sips 的输出字节逐项一致。此前 UI/试对话实现与未验证的真实 Provider/系统动态/Windows 边界保持 [实施记录](LIQUID_GLASS_IMPLEMENTATION_2026-10-04.md) 所述状态。

`bash scripts/deploy-local-macos.sh` 本轮成功从当前树完成 release（3m59s）、`.app/.dmg` 打包、安装并重启 `~/Applications/LocalAiMux.app`；进程存活，二进制 SHA-256 `a689748b914745ec9b7425ca7d6cac758ccb16babe39b1732db405440e3648b5` 与 `dist/localmux/build-info.txt` 一致，安装 ICNS 与 SVG 的 Tauri 导出一致。数据库完整性 `ok`、保存连接 2。本轮没有运行真实推理或改变连接/授权。

真实安装版已在 960×680、760×560 窗口核对原图标/统一容器/三行可识别性；最后恢复 960×680、跟随系统、API 停止。截图：[960](design/liquid-glass-chat/implementation/native-original-icons-960.jpg)、[760](design/liquid-glass-chat/implementation/native-original-icons-760.jpg)。机器记录：[original-icons-qa.json](design/liquid-glass-chat/implementation/original-icons-qa.json)；构建日志：`implementation/deploy-original-icons.log`。没有使用生成插画替代本轮软件原图。
