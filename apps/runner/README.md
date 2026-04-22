# Runner

`apps/runner` 现在是 SES Flow 的 Rust 工作流执行内核库，不再直接对外提供 HTTP 服务。

当前库主要提供：

- `Workflow Definition JSON` 定义模型
- 工作流执行引擎与节点执行器
- workflow 注册、编辑会话、运行、恢复、终止与事件流能力
- `run / snapshot / catalog / edit session` store 抽象与 PostgreSQL 实现

HTTP API、静态页面托管、CORS 与请求日志现在统一位于 `apps/backend`。

## 执行器核心原理

`runner` 的执行器本质上是一个“按节点类型分发的状态机”：

1. `WorkflowEngine` 先校验工作流定义，从开始节点拿到首次输入，然后进入执行循环。
2. 每次循环根据当前节点的 `node_type`，从 `ExecutorRegistry` 里找到对应的 `NodeExecutor` 实现，例如 `fetch / code / shell / wait / sub_workflow`。
3. 执行器拿到统一的 `NodeExecutionContext`，其中包含 `trigger`、当前 `input`、累计 `state`、运行环境 `env` 等上下文，产出标准化的 `NodeExecutionResult`。
4. 引擎把 `NodeExecutionResult.state_patch` 合并回全局 `state`，把本次执行记录写入 `timeline`，再结合 `branch_key` 解析下一条 transition，继续推进到下一个节点。

几个关键设计点：

- 节点执行和流程编排是分离的：节点只负责“如何算出输出”，是否跳转、跳到哪里由引擎统一处理。
- 所有节点都走统一返回结构：`output / state_patch / branch_key / next_signal / terminal`，这样新增节点类型时只需要实现 `NodeExecutor` 即可接入。
- `wait`、`sub_workflow` 这类可暂停节点会返回 `Waiting`，引擎会把当前 `run_id`、节点位置、状态、时间线、最后一次 signal 等信息封装成 `resume_state / snapshot`。
- `WorkflowRunner` 会把 `summary` 和 `snapshot` 持久化到 store；后续收到外部事件时，再按 `run_id` 取回 snapshot，校验事件是否匹配，并从等待节点之后继续执行。
- 引擎内部还带有最大步数保护、终止检查和运行摘要事件发射，避免死循环，也便于 backend 侧观察运行过程。

## 执行发生在哪个进程

默认情况下，工作流不是在独立的 `runner` 服务进程里执行，而是作为 Rust 库直接运行在 `apps/backend` 进程内。

- `backend` 启动时会创建 `WorkflowApp`，把 `runner` 作为库接入，而不是通过 HTTP 再转发给另一个执行进程。
- `start_workflow` 和 `resume_workflow` 会把实际执行逻辑放进 `tokio::task::spawn_blocking`，因此它通常运行在 backend 进程的后台阻塞线程池中，不占用 HTTP 请求处理主线程，但仍然属于同一个 OS 进程。
- 大多数节点都在这个进程内直接执行，例如流程编排、分支判断、`wait`、`sub_workflow`。其中 `sub_workflow` 也是在当前进程里直接调用子流程引擎，不会额外拉起服务进程。
- `fetch` 节点同样不新建 OS 进程，而是在当前进程里通过 `reqwest` 发起 HTTP 请求；如果当前线程没有 Tokio runtime，会临时创建一个当前线程 runtime 来完成请求。
- 只有少数节点会显式创建子进程：
  - `shell` 节点会通过 `std::process::Command` 启动 shell 子进程执行命令。
  - `code` 节点会启动一个 `node` 子进程来执行 JS/TS 代码。

可以把当前模型理解为：

`backend 主进程` -> `spawn_blocking 后台线程执行工作流` -> 某些节点按需派生 `shell/node` 子进程。

## 常用命令

```bash
cargo build -p runner --lib
cargo test -p runner --lib
just runner-test
```

## 调用方式

`backend` 通过 `runner::app::WorkflowApp` 直接复用库层能力，并继续对外暴露现有 `/runner-api` 契约。
