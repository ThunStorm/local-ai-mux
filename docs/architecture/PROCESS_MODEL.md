# 子进程模型

Phase 3 的 Codex Adapter 每次连接探测或推理启动一个官方 `codex app-server` stdio 子进程。每个推理使用独立临时工作目录，固定只读沙箱与拒绝审批；响应结束或请求取消时销毁进程句柄并发出 kill。这个早期实现以隔离为先，尚未实现 macOS 进程组和 Windows Job Object 的完整孙进程清理，Phase 8 必须补齐。
