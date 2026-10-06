# LocalAiMux macOS 视觉规范

状态：2026-10-04 实施基线。设计/验收依据见 [设计方案](LIQUID_GLASS_CHAT_DESIGN_2026-10-03.md)、[实施记录](LIQUID_GLASS_IMPLEMENTATION_2026-10-04.md)及 [HANDOFF](../HANDOFF.md)。本轮沿用 Tauri 2；以下像素和颜色为项目令牌，不是 Apple 官方指定数值。

## 材质与层级

保留系统标题栏与交通灯，WebView 不绘制系统窗口按钮。macOS 26+ 使用公开 `NSGlassEffectView` 将现有 WKWebView 放入 contentView，并设置公开 `underPageBackgroundColor`，不启用私有 API；消息、代码、列表和工作区使用不透明内容表面，只让导航/控制层表现玻璃。浏览器/旧系统使用 CSS 模糊或不透明回退。依据：[Apple Materials](https://developer.apple.com/design/human-interface-guidelines/materials)、[NSGlassEffectView](https://developer.apple.com/documentation/appkit/nsglasseffectview)。不能把 CSS 回退称为原生材质或 Apple 认证。

## 布局与空间

| 项目 | 实施约定 |
|---|---|
| 窗口 | 默认 960×680、最小 760×560；另验收 800×600、1120×760；不承诺手机 UI。 |
| 侧栏 | 外宽 204 px，10 px 边距中的 184 px 浮动表面、20 px 圆角；不大于 880 px 或手动收起时外宽 78 px，保留图标/aria-label。 |
| 导航 | 总览、应用与连接、试对话、API；高级：路由、会话；语言/外观/减少透明度留在底部偏好。 |
| 内容 | 工作区独立滚动、外沿 10 px、内容内沿 24 px；稳定正文底、不堆砌玻璃卡片；页面切换回页首。 |
| 试对话 | 消息最大 740 px；编辑器固定在底部，输入与发送/停止并排；检查器宽 248 px，1050 px 以下作为按需抽屉，避开底部 138 px 编辑器空间。 |
| 应用行 | 40 px 图标、名称/版本、独立状态与额度、就近连接操作；路径/诊断/移出列表收在展开区。行高按额度窗口自然增长，首屏至少 3 行可识别。 |
| 字体/间距 | 系统字体；标题 22/28、区标题 15/22、正文/控制 13/19、注释 11–12/17–18 px；间距 4/8/16/24。 |
| 控制 | 按钮/字段 32–36 px、导航 38 px；SVG 线性图标，输入焦点 2 px；禁用始终有相邻原因。 |

布局依据：[Apple Sidebars](https://developer.apple.com/design/human-interface-guidelines/sidebars)、[Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars)、[Layout](https://developer.apple.com/design/human-interface-guidelines/layout)。

## 颜色与辅助功能

浅色：页面 #EEF1F6、正文底 #FAFBFD、文字 #18202D、次文字 #5F6876、分隔 #D7DDE5、字段边界 #85909F、强调 #0068DB、选中底 #E3EEFF。深色：页面 #191D25、正文底 #1C212A、文字 #F1F3F7、次文字 #AEB6C4、分隔 #3E4655、字段边界 #748093、强调 #78B5FF、选中底 #263D5E。内容与玻璃表面分开；玻璃边缘细高光、阴影克制；不使用彩色壁纸或装饰性动态折射。

普通文字目标 ≥4.5:1、控制/焦点 ≥3:1；禁用输入可淡化，解释文字保持可读。系统减少透明度/增强对比度/减少动态效果通过主线程 AppKit 读取，每 2 秒更新；应用内减少透明度另有开关。WebView 外观与原生窗口的浅/深/系统同步；媒体查询和不透明回退保留。动效仅 120–180 ms 状态反馈，减少动态效果关闭。依据：[Apple Accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility)、[Dark Mode](https://developer.apple.com/design/human-interface-guidelines/dark-mode)。

## 图标

2026-10-05 图标策略按用户最新要求修订：LocalAiMux 使用确定性 SVG 的 AI 核心与 Mux 三入一出结构，固定路径/色值，Tauri CLI 导出各平台资源。应用内直接使用各软件已安装包/CLI 的原始图标，保留颜色/几何，只统一 40/22 px、3/1 px 留白、中性表面、细边框与圆角。无原图时用有名称的中性回退，不借用其他产品图标；上轮生成插画只作为历史，不进入生产。来源与本轮验收见 [图标修订](ICON_REFINEMENT_2026-10-05.md)。

## 交互与验收

键盘顺序与视觉一致，图标有名称提示，导航 `aria-current`、折叠原生 details、状态 role/status，消息纯文本 role/log；不只用颜色解释状态。玻璃侧栏在独立较高层叠上下文，偏好菜单不能被工作区/编辑器遮挡。窄屏检查器可关闭/Escape，焦点回参数按钮；用户向上阅读时不强拉消息滚动。

API 原有状态→地址/Key→授权型号→配置示例顺序保留，凭据显示/复制须点击。模型/额度/保存连接不冒充真实调用通过；Codex 窗口逐条展示；MiMo 开放平台、桌面与 CLI 不混同。搜索/筛选不调用推理。验收包括各尺寸、浅/深、键盘、回退、图标加载、无水平溢出、`pnpm typecheck && pnpm test`、全仓验证、macOS 当前树打包安装重启与原生检查；真实上游另列证据。
