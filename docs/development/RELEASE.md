# 发布与更新

## macOS 预发布

1. 更新 workspace Cargo、Cargo.lock、前端与 Tauri 版本，整理 CHANGELOG 和版本说明。
2. 检查候选 Git 树及提交元数据，移除本机证据、生成包、运行时数据库和凭据，运行 `bash scripts/verify.sh`、Clippy 和隔离 SDK smoke。本机安装前确认任务结束并备份数据库。
3. 推送工作分支，提交 PR，核对 CI 与桌面构建。合并后以 `main` 对应提交推送带版本说明的注释标签。`release.yml` 校验标签版本和 `main` 归属，从标签的干净工作树构建 Apple Silicon 应用。GitHub 预发布包含 `LocalAiMux-macos-arm64.zip` 和自动生成的源码归档。验收结果应与其实际验证的提交关联。

## 桌面安装包与自动更新

`desktop-build.yml` 在 PR 上构建 macOS `.app` 和 Windows NSIS 安装包，供 CI 验证。`signed-update-candidate.yml` 通过手动触发生成 Tauri 更新签名候选，需要仓库配置相应更新私钥、公钥和 HTTPS 地址。桌面按打包时注入的配置启用更新入口，检查与安装分开，安装前检查当前任务数。

正式发行仍需 macOS Developer ID 签名与公证、Windows Authenticode、可信更新托管，以及原生安装、升级、签名校验和回滚演练。见[发行待办](../TODO.md)。
