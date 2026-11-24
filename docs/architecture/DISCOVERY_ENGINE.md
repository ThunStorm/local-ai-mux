# 本机发现引擎

Phase 2 的扫描器通过静态检测清单识别 Codex、Qoder、Qoder CN、CodeBuddy Code、CodeBuddy CN、WorkBuddy、Kimi、Claude、Gemini、OpenCode、Qwen、ZCode、TRAE、Xiaomi MiMo Desktop 和 Ollama。macOS 检查 PATH、常见 CLI 目录和 `.app/Contents/Info.plist`；Windows 检查 PATH、常见安装目录、Uninstall 与 App Paths 注册表。Bundle ID 仅按完整段匹配，避免把名称相近的其他应用误报。

扫描只读取文件是否存在、应用包名称/版本和注册表安装元数据，不读取配置内容、登录凭据或 Keychain。输出 `app_id/name/version/executable/install_path/detection_source/connectability/diagnostics`。Phase 2 只标记 `DETECTED_ONLY` 或 `BROKEN_INSTALL`；连接与登录检测在对应 Adapter 阶段执行。扫描使用阻塞线程，不占用 Tauri UI 线程。

本机 WorkBuddy 包内虽有 `Contents/Resources/app.asar.unpacked/cli/bin/codebuddy` 脚本，却缺少完整公开 CLI 运行文件，因此只登记 WorkBuddy 桌面应用，不把该脚本当成可连接的模型 CLI。CodeBuddy CN.app 中的 `app/bin/code` 是编辑器启动器；独立 CodeBuddy Code CLI 只按 `codebuddy`/`cbc` 可执行文件另行发现。

Qoder 官方安装器把 `qoder` 命令放在 `~/.qoder/entry`；macOS 扫描额外检查该目录，安装后即使桌面应用未继承终端 PATH 也可发现。macOS 另查 `~/.mimocode/bin/mimo`；MiMo Code CLI、MiMo Desktop 和开放平台 API Key 是不同连接形态。发现 CLI 不等于已连接；用户点击后才启动其官方本机能力 API 并查询授权型号。
本机 Qoder.app 的 Bundle ID 为 `com.qoder.app`，Qoder CN.app 为 `com.qodercn.app`；两张卡片分开。全球版 `~/.qoder/entry/qoder` 与中国版 `~/.qoder-cn/entry/qodercn` 是不同 CLI 和配置目录，不共享连接或模型前缀。Qoder CN 官方 CLI 1.1.64 已单独安装，尚未登录；旧登记记录经 `0010` 迁移到对应应用 ID。应用主列表只显示当前实现了可用 Bridge 的 ID；扫描器的其他只读结果不再混入主列表。
