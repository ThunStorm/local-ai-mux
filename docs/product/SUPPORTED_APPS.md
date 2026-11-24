# LocalAiMux 应用支持矩阵

v0.3.0 源码基线。这里描述已实现Bridge，不把本机登录、已保存目录或历史某型号成功当作当前全部型号通过。运行时具体版本/目录随用户安装而变化，能力取当前公开Schema/型号证据。

| 应用 / Bridge | 连接与目录实现 | 文本/能力范围 | 额度与真实验收边界 |
|---|---|---|---|
| Codex / 官方App Server | 用户显式连接；动态model/list，当前安装CLI公开Schema核对 | typed流/非流、前置system/developer、按型号effort、原生Responses续接；Chat不伪造历史能力 | 官方多窗口用量；已有部分macOS真实样本，不扩大到所有型号/续接/Windows |
| Qoder / 官方CLI | 独立全球版连接，JSON/真实MODEL文本目录 | 前置system+单user，公开headless JSONL流/非流与CLI会话入口，未知参数拒绝 | 积分尚未采集；历史连接/调用记录不替代本版全部型号真实质量 |
| Qoder CN / 官方独立CLI | 独立app_id/路径/登录/目录，不继承全球版账户 | 与相应公开CLI协议映射，能力按精确身份 | 余额未知，真实连接/质量按独立验收 |
| CodeBuddy Code / 官方独立CLI | 独立CLI/NVM入口发现；显式覆盖或公开help解析，目录可PARTIAL | 前置system+单user、只读/无工具请求、JSONL与续接入口 | 不等同WorkBuddy桌面订阅；额度未知、真实工具/订阅与Windows仍待核验 |
| MiMo Code CLI / 公开本机能力API | 用户点击后临时serve、目录、限型号短时token；断开撤销 | HTTP文本能力按精确目录元数据；不读取Desktop登录态 | 临时目录/令牌已有历史只读核验，推理/续期和Windows另验 |
| MiMo开放平台 / 用户API Key | 显式输入独立Key，本次进程内存持有 | HTTP文本/完整合法消息与已报告参数 | 不共享桌面订阅；额度未知，真实账单/Key验收独立 |
| WorkBuddy / CodeBuddy CN桌面 | 只读发现/登记，不注册当前Gateway | 尚无已实现且验证的桌面Bridge | 不用编辑器启动器或私有登录态冒充模型接入；[WorkBuddy调研](../adapters/WORKBUDDY.md) |
| MiMo Desktop / Ollama / TRAE等仅检测应用 | 可发现/登记，按支持注册表进入主列表 | 本版没有相应可路由Adapter | 不因安装就开放调用 |
| GLM / Ark / Kimi组件 | 独立crate/离线测试；未接生产注册（Kimi明确Unsupported） | 以各Adapter契约为准 | 不宣称真实账户/费用/平台通过 |

所有Agent调用仍需Key显式授权；费用/用量/额度未知不当零或免费。普通可恢复错误、局部熔断、截断和预算不足不能自动升级为供应商停服结论。下一步与外部验收见[TODO](../TODO.md)，当前安装验证见[封版记录](../releases/V0_2_0_FREEZE.md)。
