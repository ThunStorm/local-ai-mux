# CI

`ci.yml` 在 macOS/Windows 运行 Rust 格式、Clippy、全工作区测试和前端类型检查/测试/构建；Ubuntu 使用官方 Python/TypeScript OpenAI SDK 对 Mock Gateway 验证模型目录、Chat、Responses 的非流式与流式协议。`desktop-build.yml` 在两个原生平台分别构建未签名 `.app` 与 NSIS 包。`signed-update-candidate.yml` 是需发布密钥的手动更新包候选流水线，不上传公开 Release。

本机 macOS 的通过结果不能替代 Windows CI 原生结果。正式发布另需系统级代码签名、公证与安装/更新实机验收。

2026-10-10：官方 SDK 门禁统一使用 `bash scripts/sdk-smoke.sh`（可用 LOCALMUX_TEST_PYTHON 指向已有 SDK Python），为两个客户端分别启动独立合成服务，固定回环 NO_PROXY，拒绝生产8317端口/非合成Key；新增length/cancel/断开/400/429/503与请求关联回归。Rust/前端默认验证不安装网络依赖；本机完整验收另跑脚本、feature-enabled harness Clippy和GUI，证据见[可靠性验收](../architecture/RUNTIME_CONTRACT_VALIDATION_2026-10-10.md)。


v0.2.0：verify.sh/verify.ps1与主CI增加`node scripts/check-release.mjs`，先核对workspace/Cargo.lock/前端/Tauri版本、稳定bundle identity、废弃品牌及Markdown本地目标。全量Rust与Clippy仍包含Unix owned进程/元数据/导出、前端类型/交互状态测试；SDK仅使用隔离合成服务。远端结果须绑定本次提交，Windows/ARM64实际安装仍独立待验，不能由某平台CI绿色推定所有原生能力通过。当前证据集中记录于[封版](../releases/V0_2_0_FREEZE.md)。

2026-10-26远端补验：候选852037e的Ubuntu SDK job在Cargo编译前由Rustup自动补装rustfmt时发生bin/cargo-fmt文件冲突。SDK/打包/签名候选工作流现与verify job一致显式准备rustfmt、clippy，避免首次Cargo再补装；Python SDK通过tests/compatibility/requirements.txt固定3.24.0，与本机和冻结基线一致，不将未固定的最新SDK当作同一验证环境。最终结果以修复后提交的Checks为准。


## 封版后Windows存储测试维护（2026-10-26）

用户报告17分6秒的失败对应dec15ee / [CI40280603119](https://github.com/ThunStorm/local-ai-mux/actions/runs/40280603119)，Windows job121432822644在migration/diagnostics断言完成后，删除临时DB时发生Windows OS32共享冲突。封版提交1d0f031随后同代码通过，说明存在偶发清理竞态，不能只凭一次绿色忽略。

全部六个文件型存储测试改用独立tempdir；重开前close并drop旧池，结束时统一关闭/释放后清理。只对Windows OS32/33共享或锁冲突在5秒内异步退避重试，其他错误及超时仍使测试失败；不把remove_file错误改为忽略。新增cfg(windows)测试真实持有不允许删除的文件句柄，释放后确认清理成功。生产open/迁移/连接策略不变，不修改用户数据库；v0.2.0标签保持不动，此为分支Unreleased维护。

本机storage13/13、全仓Clippy通过；Windows删除锁回归及远端完整三项检查以本次维护提交结果为准。
