# LocalAiMux — Agent 导航

当前交付见 `docs/HANDOFF.md`，架构入口为 `docs/architecture/OVERVIEW.md`，后续工作见 `docs/TODO.md`。按需读取文档；不要全库扫描 Markdown。真实 Provider 仅使用稳定公开接口，连接须用户显式发起。

## 运行

- 安装：`pnpm install`、`rustup show`
- 验证：`bash scripts/verify.sh`（Windows：`pwsh scripts/verify.ps1`）
- 桌面开发：`pnpm dev`

## 硬约束

- 最终产品是 Tauri 2 桌面应用；默认 API 仅监听 `127.0.0.1:8317`。
- Agent、Provider、Model、Account 分离；本阶段以真实模型 ID 为主，旧 free 路由仅兼容保留，费用或额度不确定为 `UNKNOWN`。
- Secret 不进 SQLite 明文或日志；第三方凭据必须由用户显式连接。
- 每个 Phase 同步编译、测试、相关文档与 `CHANGELOG.md` Unreleased。

## 任务路由

| 任务 | 必读 | 可改 | 必跑测试 |
|---|---|---|---|
| 恢复工作 | `docs/HANDOFF.md` | — | — |
| 产品/范围 | `docs/BRIEF.md`, `docs/LOCALMUX_PROJECT_PLAN.md` | `docs/product/` | — |
| 桌面 UI | `docs/product/UX_SPEC.md`, `docs/product/MACOS_VISUAL_SPEC.md`, `docs/development/BUILD.md` | `apps/desktop/src/` | `pnpm typecheck && pnpm test` |
| 存储/目录 | `docs/architecture/DATABASE.md` | `crates/storage/`, `crates/core/`, `migrations/` | `cargo test -p localmux-storage -p localmux-core` |
| 日志 | `docs/architecture/OBSERVABILITY.md` | `crates/telemetry/` | `cargo test -p localmux-telemetry` |
| 打包/CI | `docs/development/CI.md`, `docs/development/BUILD.md` | `.github/workflows/`, `scripts/` | `bash scripts/verify.sh` |
| 架构与 Adapter 契约 | `docs/architecture/OVERVIEW.md`, `docs/architecture/ADAPTER_SYSTEM.md` | `crates/domain/`, `crates/adapter-sdk/` | `cargo test -p localmux-domain -p localmux-adapter-sdk` |
| Gateway API | `docs/api/OPENAI_COMPAT.md`, `docs/api/ERROR_MODEL.md`, `docs/api/STREAMING.md` | `crates/gateway/`, `crates/api-types/` | `cargo test -p localmux-gateway` + SDK smoke |
| Discovery | `docs/architecture/DISCOVERY_ENGINE.md` | `crates/discovery/` | `cargo test -p localmux-discovery` |
| Adapter | `docs/architecture/ADAPTER_SYSTEM.md`, `docs/adapters/<NAME>.md` | `crates/adapters/<name>/` | 对应 crate 测试与离线 fixture |
| Router/额度/会话 | 对应 `docs/architecture/` 文档 | `crates/router/`, `crates/quota/`, `crates/sessions/` | 对应 crate 测试 |

## 每次改动的最短闭环

1. 先读 `docs/HANDOFF.md`，按上表只读本次任务对应文档；涉及范围或阶段时才读 `docs/BRIEF.md` / `docs/LOCALMUX_PROJECT_PLAN.md`，追溯决策时才读 `docs/DECISIONS.md` 相关条目。动手前用一句话明确验收条件。
2. 只改相关代码与说明；行为或契约变化同步对应文档和 `CHANGELOG.md` 的 Unreleased，实际进度或阻塞变化同步 `docs/HANDOFF.md`。不为纯格式改动扩写文档。
3. 跑上表对应测试；每次改动后在 macOS 执行 `bash scripts/deploy-local-macos.sh`，从当前工作树编译、安装到 `~/Applications/LocalAiMux.app`、退出旧版并重启，确认进程仍在。Windows 需完成等价本机安装和重启；无法执行时记录未验证项。应用仍有任务运行时先等其结束，不能强制重启。
4. 验收通过后按需提交、推送：用户要求交付远程或需要共享里程碑时提交并推送；否则保留本地改动并在交接中说明。推送前检查 diff、状态与目标分支，不自动发布 Release。

不要把进度写进本文件；更新 `docs/HANDOFF.md`。不要创建平行项目结构或用 GUI 自动化接入 Provider。
