# 本机发现引擎

Phase 2 的扫描器通过静态检测清单识别 Codex、Qoder、CodeBuddy/WorkBuddy、Kimi、Claude、Gemini、OpenCode、Qwen、ZCode、TRAE、Xiaomi MiMo Desktop 和 Ollama。macOS 检查 PATH、常见 CLI 目录和 `.app/Contents/Info.plist`；Windows 检查 PATH、常见安装目录、Uninstall 与 App Paths 注册表。Bundle ID 仅按完整段匹配，避免把名称相近的其他应用误报。

扫描只读取文件是否存在、应用包名称/版本和注册表安装元数据，不读取配置内容、登录凭据或 Keychain。输出 `app_id/name/version/executable/install_path/detection_source/connectability/diagnostics`。Phase 2 只标记 `DETECTED_ONLY` 或 `BROKEN_INSTALL`；连接与登录检测在对应 Adapter 阶段执行。扫描使用阻塞线程，不占用 Tauri UI 线程。

macOS 的 WorkBuddy 应用包若包含 `Contents/Resources/app.asar.unpacked/cli/bin/codebuddy`，另列出该内置 CLI，用户点击连接后才运行。该脚本依赖 Node.js 18.20.8 及以上；扫描不验证登录或运行时。Qoder 桌面包和 MiMo 桌面包没有因此被当作 CLI。

Qoder 官方安装器把 `qoder` 命令放在 `~/.qoder/entry`；macOS 扫描额外检查该目录，安装后即使桌面应用未继承终端 PATH 也可发现。MiMo Code CLI 与 MiMo Desktop/API Key 属不同连接形态，当前不注册到 Gateway。
