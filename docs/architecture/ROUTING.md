# 路由与免费池

Phase 6 的 exact route 使用 `provider/model`；虚拟路由为 `free`、`cheap`、`balanced`、`best`。路由只从已连接 Adapter 当前上报的模型描述构造，不写死免费型号。目录缓存 5 分钟，用户刷新在 30 秒内合并，限流后立即使受影响连接的目录失效。

`free` 仅选择 `CostMode::FREE`；`RESERVE` 还需用户开启 `reserve_as_free`，并有 10 分钟内的上游报告余额大于 0。`UNKNOWN`、`METERED` 永不进入 free。无候选返回 HTTP 429 与 `LMX_FREE_POOL_EXHAUSTED`，不会偷偷转收费模型。

`cheap` 顺序为 FREE → 已确认可用 RESERVE → SUBSCRIPTION，同层按上游倍率、用户优先级、模型 ID 排序；倍率未知会排在同层最后。`balanced` 必须先由用户填写优先级；计费模型还需显式开启 `allow_metered`。`best` 仅指向用户指定的 exact model，不自动评判“最佳”。授权失败、离线、不支持和已熔断连接会被跳过。429/额度耗尽触发短期熔断，到期后允许半开尝试，成功后关闭熔断。

同一 Conversation 的黏性路由在 Phase 7 实现。当前所有真实 Adapter 的费用分类都是 UNKNOWN，因此默认 free 池为空；测试中的动态免费示例只是合成 Adapter，不代表真实供应商已报告免费能力。
