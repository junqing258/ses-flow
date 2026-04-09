# Runner

`apps/runner` 是仓储/物流分拣作业编排平台的自研 DSL Runner 原型，采用 Rust 实现。

当前版本提供：

- 稳定的 `Workflow Definition JSON` 定义模型
- 最小可运行的执行内核
- 内置基础节点执行器
- `connector / action / task` handler registry
- `waiting -> resume` 恢复执行能力
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

## Current Node Support

- `start`
- `end`
- `fetch`
- `set_state`
- `switch`
- `action`
- `wait`
- `task`

当前的 `fetch`、`action`、`wait`、`task` 仍是受控 stub，用于先把定义层、状态模型和节点协议跑通。
其中 `fetch / action / task` 已经改为通过 registry 分发，后续接真实外部系统时只需要替换对应 handler。
