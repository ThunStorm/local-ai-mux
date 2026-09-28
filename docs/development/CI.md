# CI

`ci.yml` 在 macOS/Windows 运行 Rust 格式、Clippy、全工作区测试和前端类型检查/测试/构建；Ubuntu 使用官方 Python/TypeScript OpenAI SDK 对 Mock Gateway 验证模型目录、Chat、Responses 的非流式与流式协议。`desktop-build.yml` 在两个原生平台分别构建未签名 `.app` 与 NSIS 包。`signed-update-candidate.yml` 是需发布密钥的手动更新包候选流水线，不上传公开 Release。

本机 macOS 的通过结果不能替代 Windows CI 原生结果。正式发布另需系统级代码签名、公证与安装/更新实机验收。
