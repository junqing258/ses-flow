# Runner

`apps/runner` 现在是 SES Flow 的 Rust 工作流执行内核库，不再直接对外提供 HTTP 服务。

当前库主要提供：

- `Workflow Definition JSON` 定义模型
- 工作流执行引擎与节点执行器
- workflow 注册、编辑会话、运行、恢复、终止与事件流能力
- `run / snapshot / catalog / edit session` store 抽象与 PostgreSQL 实现

HTTP API、静态页面托管、CORS 与请求日志现在统一位于 `apps/backend`。

## 常用命令

```bash
cargo build -p runner --lib
cargo test -p runner --lib
just runner-test
```

## 调用方式

`backend` 通过 `runner::app::WorkflowApp` 直接复用库层能力，并继续对外暴露现有 `/runner-api` 契约。
