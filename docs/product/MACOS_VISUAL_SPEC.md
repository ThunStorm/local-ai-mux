# LocalMux macOS 视觉规范

状态：2026-09-28 实施基线。范围为 Tauri 原生窗口中的总览、应用与连接、API、路由和会话；不改变 Provider 授权、模型能力或旧路由语义。以 [产品对齐计划](../LOCALMUX_PRODUCT_ALIGNMENT_PLAN_2026-09-28.md)和[交接记录](../HANDOFF.md)为事实来源。

## 设计原则

1. **像 Mac 工具，而非网页首页。** 保留系统标题栏与窗口按钮，内容层使用固定侧边导航、每页紧凑标题和就近操作；不在 WebView 重画交通灯。侧边栏用于顶层区域，当前页标题与扫描/启停操作留在内容工具栏。依据：[Apple Sidebars](https://developer.apple.com/design/human-interface-guidelines/sidebars)、[Apple Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars)、[Apple Windows](https://developer.apple.com/design/human-interface-guidelines/windows)。
2. **先展示可行动事实。** 默认 800×600 窗口首屏应看到当前页标题、状态、主操作和至少三条应用；路径、诊断和高级 Key 权限在行内展开。只有官方报告的模型与额度进入主信息层；「已保存连接」不写成「已验证可调用」。
   总览在统计条下展示已保存连接和下一步动作，避免大面积无信息留白；页面切换重置内容滚动位置。
3. **用列表承载应用。** 同一应用的桌面包与 CLI 在一行，支持搜索和状态过滤；行内排列名称、版本/路径、连接状态、额度摘要与操作。MiMo Desktop 与 MiMo 开放平台 API Key 分区展示。无官方桥时仍可登记应用，但不出现伪连接按钮。
4. **API 配置是一条顺序路径。** 运行状态/启停 → Base URL → 遮罩 Key → 真实模型 → 配置示例。未运行时地址可以查看但明确标「未启动」；显示/复制 Key 必须用户点击；模型测试明确提示会发起上游请求。高级 Key 管理和桌面更新设置折叠在主流程后。macOS 的关键控制不依赖窗口底部才可触达，依据：[Apple Layout](https://developer.apple.com/design/human-interface-guidelines/layout)。

## 布局与组件

| 项目 | 规范 |
|---|---|
| 窗口 | 默认 960×680，最小 760×560；WebView 占满可用区域，系统标题栏保留。800×600 必须无水平滚动。 |
| 侧边栏 | 常规宽 188 px；窗口不大于 880 px 时收为 58 px 图标栏，按钮保留 `aria-label` 与悬停提示。主导航：总览、应用与连接、API；「高级」下为路由、会话。 |
| 内容 | 独立垂直滚动；每页 22–28 px 内边距，列表与分隔线填满内容宽度。标题栏与页面操作同一行。 |
| 字体 | `-apple-system, BlinkMacSystemFont, system-ui`；页标题 24/30 px、区标题 16/22 px、正文与控件 13/19 px、注释 12/17 px；数字与路径使用等宽数字/系统等宽字体。 |
| 间距 | 基础 4 px；行内 8 px、模块 16 px、区段 24 px。应用行常态约 82 px，展开诊断后自然增高。 |
| 控件 | 常规按钮/输入高 32 px，侧边导航目标至少 36 px；主要操作使用实心强调色，次要操作为浅边框，破坏性操作用明确文字而不只用红色。 |
| 表面 | 内容以开放列表和细分隔线为主；仅空态、提示和代码示例使用浅色面板。不要为每条应用或每个数值套大卡片。 |

### 色彩令牌

轻外观：页面 `#F7F9F8`、侧边栏 `#EDF2EF`、内容 `#FCFDFC`、主文字 `#202925`、次文字 `#52635B`、分隔线 `#DCE4DF`、强调 `#176C6B`、强调浅底 `#DDEDE8`。深外观：页面 `#151C1A`、侧边栏 `#1D2824`、内容 `#202A26`、主文字 `#F2F6F3`、次文字 `#B6C9BD`、分隔线 `#35443B`、强调 `#6CCFC0`、强调浅底 `#23413A`。状态文本始终与图形/颜色同时出现；错误与警告使用独立语义色。系统外观为默认值，浅/深模式分别验证对比度与焦点。依据：[Apple Color](https://developer.apple.com/design/human-interface-guidelines/color)、[Apple Dark Mode](https://developer.apple.com/design/human-interface-guidelines/dark-mode)、[Apple Accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility)。这些数值是 LocalMux 实施令牌，不是 Apple 的官方尺寸或色值。

## 交互与无障碍

- 应用搜索按名称和安装路径过滤；状态筛选是纯前端视图，不发起 Provider 调用。扫描、添加、断开和 Key 操作保持原有 Tauri 命令边界。
- `Tab` 顺序与视觉顺序一致；所有图标按钮有文字或 `aria-label`，选中导航使用 `aria-current="page"`；折叠内容使用原生 `details/summary`。焦点环可见，状态更新使用 `role="status"` 或 `aria-live`，不只靠颜色表达。
- 长路径单行省略并在展开区完整显示；额度窗口逐条列出单位、来源、更新时间/过期与重置时间，不相加。
- 动效仅用于悬停和状态反馈（120–180 ms），遵守 `prefers-reduced-motion`；禁用按钮须给出相邻原因。透明/模糊效果不是依赖，减少透明度时仍清晰。

## 验收清单

- 960×680、800×600、760×560 三种窗口尺寸下无水平滚动、无截断主操作；页面内可滚动而侧边导航保持定位。
- 应用页首屏至少三条可识别行；Codex 双额度独立；MiMo Desktop 与 MiMo API 分区；搜索和状态筛选不改连接。
- API 页首屏出现运行状态、Base URL、Key 与模型行；未启动/无模型/Key 丢失状态各有说明；复制完整配置仍需用户点击。
- 浅色、深色、键盘焦点和窄窗口均实测；`pnpm typecheck && pnpm test`、`bash scripts/verify.sh` 与 macOS 本机安装重启通过。
