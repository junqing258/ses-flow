# Runner

`apps/runner` 是仓储/物流分拣作业编排平台的自研 DSL Runner 原型，采用 Rust 实现。

当前版本提供：

- 稳定的 `Workflow Definition JSON` 定义模型
- 最小可运行的执行内核
- 内置基础节点执行器
- `connector / action / task` handler registry
- `run / snapshot` store 抽象与内存实现
- `waiting -> resume` 恢复执行能力
- `resume` 事件类型与关联键校验
- 示例流程与本地运行入口

## Commands

```bash
cargo run -- --workflow examples/sorting-main-flow.json
cargo test
```

等待态恢复执行：

```bash
cargo run -- --workflow examples/sorting-main-flow.json > /tmp/runner-waiting.json
cargo run -- --workflow examples/sorting-main-flow.json --resume-state /tmp/runner-waiting.json --event examples/rcs-callback.json
```

当前恢复校验规则：

- `wait` 节点会校验 `event/type` 是否匹配节点配置的 `config.event`
- 如果等待信号里包含 `correlationKey`，恢复事件必须带相同的 `correlationKey`
- `task` 节点默认要求 `event/type = task.completed`
- 如果任务创建信号里包含 `taskId`，恢复事件必须带相同的 `taskId`

## Current Node Support

- `start`
- `end`
- `webhook_trigger`
- `respond`
- `fetch`
- `set_state`
- `if_else`
- `switch`
- `sub_workflow`
- `action`
- `command` (`action` alias)
- `wait`
- `task`

当前的 `fetch`、`action`、`wait`、`task` 仍是受控 stub，用于先把定义层、状态模型和节点协议跑通。
其中 `fetch / action / task` 已经改为通过 registry 分发，后续接真实外部系统时只需要替换对应 handler。
当前 `run store` 为内存实现，已经把引擎与持久化边界拆开，后续可以替换为 PostgreSQL / Redis / KV 存储实现。
当前 `sub_workflow` 支持同步执行；如果子流程进入 `waiting`，父流程会返回失败，父子级联恢复还没有实现。
