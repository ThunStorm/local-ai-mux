# 发布与更新

日常 `desktop-build.yml` 在 macOS 产出 `.app`、Windows 产出 NSIS 安装包，作为未签名 CI 验证包，不自动发布。`signed-update-candidate.yml` 仅在手动触发时运行，并要求仓库配置 `TAURI_SIGNING_PRIVATE_KEY`、`LOCALMUX_UPDATER_PUBLIC_KEY` 和 HTTPS `LOCALMUX_UPDATER_ENDPOINT`；使用 `tauri.release.conf.json` 生成带签名的 Tauri 更新包。私钥只进入 CI 密钥，不进入仓库。桌面更新按钮只在公钥及 HTTPS 地址随构建注入后启用；检查和安装分成两次明确点击，安装前检查当前任务数。

更新包签名不等于操作系统发行签名。macOS Developer ID 签名/公证、Windows Authenticode 证书、更新托管地址和真实安装回滚演练尚未配置，因此目前没有正式发布。取得这些外部材料后，应在两种系统上验证安装、首次启动、托盘、登录自启、检查更新、下载验证和失败恢复，再发布版本。
