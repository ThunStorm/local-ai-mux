# 交接记录

分支 `codex/phase0-main` 从 `main` 的 `94aec3f` 独立建立，不包含 `codex/dev` 开发文件。Phase 0–8 的可离线实现已按顺序推进；真实 Provider、发行签名和 Windows 实机验收仍按下述边界保留。

Phase 1 已实现：领域类型、Adapter SDK、Mock echo、SQLite Key 哈希仓储、SecretStore 抽象、loopback Axum Gateway、Tauri API 启停与 Key 管理命令。官方 Python/TypeScript OpenAI SDK 已通过 Mock 模型列表、非流式聊天与 SSE 流式聊天。

Phase 1 验证：`cargo test --workspace`、Clippy `-D warnings`、前端类型检查/测试/构建、macOS Tauri debug 原生构建均通过。官方 Python/TypeScript SDK 的模型列表、非流式和流式测试通过；CI 已加入 SDK 消费端作业。

Phase 2 已实现只读发现引擎、macOS/Windows 检测清单、Discover 页面、首次使用引导、重扫和诊断。本机 macOS 扫描发现 Codex、Qoder、CodeBuddy/WorkBuddy、TRAE 候选，修复了 Bundle ID 子串误报。Windows 分支交叉编译通过；Windows CI 实机结果待验证。新工作树的 Tauri `.app` 已实际打开并显示发现列表；界面启动 API 后 `/api/health` 返回 200、无 Key 的 `/v1/models` 返回 401，停止后端口关闭。

Phase 3 代码已实现 Codex 官方 App Server 接入、显式添加、路径校验、动态模型目录、只读调用、增量 SSE 与线程续接；离线 fixture 测试通过。Codex Adapter 的 Windows 交叉目标 `cargo check`、工作区 Clippy、macOS `.app` 构建和 Mock Gateway 官方 Python/TypeScript SDK 复验通过。新桌面 Discover 页已实际显示 Codex CLI 的「版本未验证」及可点击「添加」。本机 Codex CLI 为 alpha 版本，尚未由用户在 LocalMux 内显式连接，真实 SDK→Codex 请求仍是未完成验收项。当前不得宣称 Phase 3 的真实 Provider 验收通过。

Phase 4 代码已按 Qoder→CodeBuddy 顺序实现：官方 headless CLI 参数、共享 JSONL 进程封装、动态目录保守解析、隔离只读调用和会话 ID。全工作区测试、Clippy、前端检查、macOS `.app` 构建及 Qoder/CodeBuddy Windows 目标 `cargo check` 通过；离线三 Adapter 目录/精确路由测试通过。新桌面仍把 Qoder/CodeBuddy 的桌面应用显示为“仅检测到”，没有伪造 CLI 连接。本机没有 Qoder/CodeBuddy CLI，真实三上游同时连接验收仍缺失。

Phase 5 已完成隔离代码：Kimi Adapter 明确 Unsupported；GLM/Ark Coding Plan Provider 组件固定专用地址、仅内存持有 Key、禁用重定向，本地模拟 HTTP 测试通过。工作区 Rust 测试、Clippy、前端检查和 macOS Tauri debug 构建通过。macOS 主机的 Windows 交叉检查在 `ring` 的 C 编译阶段因缺少 Windows 目标标准头文件失败，未触及本项目 Rust 代码；仍需 Windows CI 原生验证。三者未注册桌面 Gateway，没有真实 Key、模型目录或账单验证，故 Phase 5 真实验收未完成。

Phase 6 已实现额度来源/快照、按连接刷新去抖、持久路由策略、free/cheap/balanced/best 动态选择、Reserve 余额门槛和熔断。全工作区测试、Clippy、前端检查、macOS `.app` 构建及 Windows 目标 Router/Quota 静态检查通过。新桌面路由页已实际显示无候选/官方额度未知，Reserve 开关交互正常。合成 Adapter 测试证明 free 分类变化后刷新生效、无免费时返回 429，绝不改走收费模型。真实供应商仍未上报费用或余额，默认 free 池为空。

Phase 7 已实现 Conversation/ProviderSession/Response ID 映射、Responses 文字非流式与 SSE、`previous_response_id` 的最新响应续接、黏性路由和拒绝跨 Provider 误续接的审计事件。全工作区测试、Clippy、前端检查和 macOS `.app` 构建通过；官方 Python/TypeScript SDK 对 Mock 路径的 Responses 测试通过，离线 Codex fixture 完成两轮续接和流式事件测试。新桌面 Sessions 页空态已实际验证。SQLite 不保存提示词/回答正文。真实 Provider 续接仍待用户显式连接验收。

Phase 8 已实现 macOS Keychain / Windows Credential Manager `SecretStore`、只存引用的 schema、API Key 的路由/连接/执行权限与每 Key 限流。默认 Key 为 `INFERENCE_ONLY`，Agent 需 GUI 显式选只读并取得 `agent:invoke`；无法落实的写入/完整 Agent Profile 被拒绝。Chat、Responses 与模型目录均校验权限；Responses 会话绑定创建它的 Key，升级前归属未知的旧会话不能经 API 续接。新增安全回归测试覆盖 Agent 拒绝与显式放行、跨 Key 会话拒绝、路由/连接限制、限流、1 MiB 请求体和撤销。日志输出统一脱敏，生产 CSP 限制来源。

Phase 8 桌面端加入已有 Key 管理、托盘 API 状态/启停/复制地址/任务与警告数、登录自启和关闭到托盘的显式开关。未配置可信公钥及 HTTPS 更新源时，检查更新不可用；手动签名更新候选流水线不发布正式版本。完整工作区测试、Clippy、前端类型检查/测试/构建与 macOS `.app` 构建通过；官方 Python/TypeScript SDK 的 Mock 模型、Chat、Responses 与流式路径复验通过。新 `.app` 已打开，API 页面和只读 Agent 警示可见，GUI 启停 API 与 loopback/401 已复核。托盘菜单自动化无法从当前系统辅助功能树读取，仍需人工点击复核。

待外部验收：用户在 GUI 显式连接 Codex 后进行真实订阅调用；安装 Qoder/CodeBuddy 官方 CLI 后做三上游同时运行；GLM/Ark/Kimi 的账户与资费/权限核实；Windows 原生 CI/安装；系统凭据库实际读写；macOS/Windows 发行证书、公证、HTTPS 更新托管和安装/回滚演练。在这些证据齐备前不能宣布正式发布或真实 Provider 验收完成。下一开发阶段为 Phase 9 P1 Adapter，须遵守用户另行授权和既有连接边界。

2026-09-27：新增最短改动闭环（见 `AGENTS.md`）和 `bash scripts/deploy-local-macos.sh`。脚本从 `codex/phase0-main` 工作树打包，安装到 `~/Applications/LocalMux.app` 并重启；`dist/localmux/build-info.txt` 记录分支、提交号及未提交状态。本轮 Rust 工作区测试、前端类型检查/测试/构建、macOS release 打包、进程和安装版哈希校验通过；`cargo fmt --check` 复测通过。锁定工具链修复遇到 rustup 并发冲突，改用同为 1.98.1 的已安装 stable 工具链及本地依赖镜像完成验证和构建。另有并行开发改动未由本轮提交或推送。

本轮应用添加修复从 `/Volumes/D/Projects/Codework/Git/local-ai-mux-phase0` 构建：Xiaomi MiMo Desktop 与 Ollama 已加入发现清单；本机 Discover 页完成 8 个桌面应用登记，正确数据库的 `added_applications` 为 8 条、`connections` 为 0 条。登记不读取桌面登录态，也不注册 Gateway Adapter；MiMo Desktop 真实上游调用仍未验收。`bash scripts/verify.sh` 通过，部署脚本的构建记录现额外保存源码绝对路径和主程序 SHA-256。

2026-09-27 后续连接开发：扫描器现识别 WorkBuddy 包内的 `codebuddy` CLI；本机只执行了不读取登录态的 `--version`，结果 2.147.0。MiMo 开放平台新增显式 API Key 连接入口，固定官方 HTTPS 端点，先验证模型目录，再将 Key 存系统凭据库并把固定引用存 SQLite；尚无用户 Key，真实 API 验收待用户在应用内发起。Qoder 桌面包未找到独立 CLI，界面提供官方安装说明；Qoder 真实连接仍待用户安装和登录。Codex 增加官方 App Server 用量窗口读取，只有后端确认 ordinary usage 可用才显示剩余百分比，不能当 Credits 余额。额度页逐连接显示官方上报或未知，并给出 Qoder、WorkBuddy、MiMo 官方查看入口；目前未发现后三者个人账户可稳定机器读取的公开余额接口，不能宣称自动余额采集已验收。后续验证结果与本机部署记录见本段追加更新。

本轮 `bash scripts/verify.sh`（本地依赖镜像与 stable 1.98.1）通过：Rust 全工作区、前端类型检查、测试和构建均成功；新增 WorkBuddy 包内 CLI 与 Codex 额度 fixture 测试通过。安装版原生窗口显示 WorkBuddy CLI、MiMo API Key 输入与 Qoder 安装指引；现有 Codex 连接点击刷新后，`account/rateLimits/read` 确实返回 300 分钟窗口的剩余百分比及重置时间。MiMo 连接记录/系统凭据的失败处理已收紧，失去凭据时不展示虚假已连接状态。没有触发 WorkBuddy/Qoder/MiMo 的真实连接，也没有读取这些桌面应用的私有登录态。最终 `bash scripts/deploy-local-macos.sh` 成功安装并重启 `~/Applications/LocalMux.app`；`dist/localmux/build-info.txt` 来源为本工作树，安装版与构建记录的主程序 SHA-256 一致（`a1c9ad40ce553ef828ecc8261edf00a1d27c4eb1cef2fa14dc7a4d4de8e4336d`）。

2026-09-28 CLI 安装：依据官方 macOS 安装文档，Qoder CLI 1.1.64 安装于 `~/.qoder/entry/qoder`（版本化二进制在 `~/.qoder/bin/qodercli/`），MiMo Code CLI 0.1.15 安装于 `~/.mimocode/bin/mimo`；两者 `--version` 均通过，未启动登录或推理。WorkBuddy 包内已有可执行 CLI，按用户要求未安装独立 CodeBuddy Code；此前仅添加的 Homebrew tap 已移除。Codex 与 Ollama CLI 原本已在本机。TRAE Agent 是需独立模型配置的另一产品，未安装。LocalMux 发现清单补入 Qoder 官方入口目录，发现测试 4/4 通过；`bash scripts/deploy-local-macos.sh` 已从本工作树安装并重启，安装版哈希与构建记录一致（`ace526ee866718091eed183be9e9edcf3395cce29d71c9faa660a1ddf3c33086`），原生 Discover 页显示 Qoder CLI 可点击「添加」。MiMo Code CLI 安装不等于 MiMo Desktop 或开放平台 API Key 在 LocalMux 中已连接。

2026-09-28：推送 `b26cc78` 后，CI 的 macOS verify 与 Ubuntu SDK smoke 通过；Windows verify 在 `cargo clippy --workspace --all-targets -- -D warnings` 停止，原因是 `security_policy.rs` 的 Unix 专用 fixture 测试留下 Windows 上未使用的 import 和辅助函数。提交 `496e5ba` 加 `#[cfg(unix)]` 后，Windows Clippy 通过，但 Discovery 测试仍用无扩展名的 Unix fixture，而 Windows 扫描 `.exe/.cmd/.bat`，导致断言失败。提交 `3f14f30` 改用扫描器的 `candidates()` 生成平台测试文件名；CI 运行 #3 的 Windows/macOS verify 与 Ubuntu SDK smoke 均通过。Windows 原生安装和实际运行仍待验收。本机部署记录为 `3f14f30`，应用进程仍在运行。
