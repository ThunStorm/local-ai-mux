# LocalAiMux

LocalAiMux **v0.3.0** 是 Tauri 2 桌面应用，将用户显式授权的官方AI运行时接入本机OpenAI兼容文本API。主流程是扫描应用、连接运行时、查看真实型号与能力、启动API并复制配置。发现、已连接、目录已报告与真实模型验证分别呈现，额度或用量无证据时保持未知。[文档导航](docs/README.md) · [支持矩阵](docs/product/SUPPORTED_APPS.md) · [架构总览](docs/architecture/OVERVIEW.md) · [版本说明](docs/releases/V0_3_0.md) · [TODO](docs/TODO.md)。

## 使用

1. 打开应用并扫描，按需连接支持的官方CLI/运行时；不读取第三方私有登录态。
2. 启动`http://127.0.0.1:8317/v1`，按需显式授予只读Agent权限并选择真实型号。LocalAiMux Key只认证本网关，供应商Key是独立连接。
3. API页“调用限时”可保存CLI空闲 **30–1200秒**，默认180秒、已有配置保留、重启有效、新请求采用；Gateway从入站起总期1200秒。试对话和HTTP Adapter仍独立120秒，不能由CLI配置推定完整链路可等20分钟；具体[计时层](docs/api/STREAMING.md)可查询。
4. 可选开启新调用正文录制；会话按来源/状态/型号/会话ID/本地时间段/已录正文筛选，全部匹配结果可保存JSON、MD、XLSX、CSV、TXT、HTML。导出可含输入与回答，部分/停止录制明确标注；未录正文不补造。

5. 偏好设置中的“数据与日志”可查看磁盘用量，设置下一次启动使用的数据目录与日志保留策略。迁移成功后旧目录保留为静态副本。恢复操作见[数据治理](docs/architecture/DATA_LIFECYCLE.md)。

Chat/Responses按精确能力支持前置指令、合法参数、流/非流与元数据；不支持的输入明确报错。`store:false`优先禁止本次本地保存。详情见[API](docs/api/OPENAI_COMPAT.md)、[会话与导出](docs/architecture/SESSION_MODEL.md)、[安全](docs/architecture/SECURITY.md)。

## 开发与验证

准备Rust1.98.1、Node22.18+/24、pnpm11.19.0和[Tauri系统依赖](https://v2.tauri.app/start/prerequisites/)，运行：

```sh
pnpm install --frozen-lockfile
pnpm dev
bash scripts/verify.sh
cargo clippy --workspace --all-targets -- -D warnings
bash scripts/sdk-smoke.sh
```

SDK脚本只运行隔离合成服务，需安装官方Python SDK，可用`LOCALMUX_TEST_PYTHON`指定环境。Windows用`pwsh scripts/verify.ps1`。本机安装与备份步骤见[构建说明](docs/development/BUILD.md)，Agent导航见[AGENTS](AGENTS.md)。

## 发布与安装边界

v0.3.0 是 macOS Apple Silicon 预发布，GitHub 提供应用压缩包和源码归档。该构建尚未通过 Developer ID 签名及公证。自动更新与正式发行条件见[发布说明](docs/development/RELEASE.md)。升级沿用 `LocalMux` 默认数据目录和 `app.localmux.desktop` 应用标识。当前验证状态见[交接](docs/HANDOFF.md)，后续工作见[TODO](docs/TODO.md)。
