# Runner

`apps/runner` 是仓储/物流分拣作业编排平台的自研 DSL Runner 原型，采用 Rust 实现。

当前版本提供：

- 稳定的 `Workflow Definition JSON` 定义模型
- 最小可运行的执行内核
- 内置基础节点执行器
- `connector / action / task` handler registry
- workspace + workflow 注册能力
- `run / snapshot` store 抽象与 PostgreSQL 持久化实现
- `waiting -> resume` 恢复执行能力
- `sub_workflow` 父子级联恢复
- `resume` 事件类型与关联键校验
- HTTP API + SSE 运行状态推送

## Database Setup

该项目使用 PostgreSQL 作为持久化存储。详细的 PostgreSQL 设置说明请参考 [POSTGRES_SETUP.md](./POSTGRES_SETUP.md)。

### 快速启动（使用 Docker Compose）

```bash
# 启动 PostgreSQL
docker-compose up -d

# 运行 runner
cargo run -- --host 127.0.0.1 --port 3002
```

### Commands

```bash
pnpm run dev
pnpm run build
pnpm run test

# or run only the runner tasks
pnpm exec moon run runner:dev
pnpm exec moon run runner:build
pnpm exec moon run runner:test

# direct cargo commands still work
cargo watch -x 'run -- --host 127.0.0.1 --port 3002'
cargo run -- --host 127.0.0.1 --port 3002

# with custom PostgreSQL database URL
cargo run -- --host 127.0.0.1 --port 3002 --database-url "postgresql://user:password@localhost/dbname"

# or use environment variable
export DATABASE_URL="postgresql://user:password@localhost/dbname"
cargo run -- --host 127.0.0.1 --port 3002

cargo test
```

默认使用 PostgreSQL 持久化存储，数据库连接字符串默认为 `postgresql://runner:runner@localhost/runner`。可以通过 `--database-url` 参数或 `DATABASE_URL` 环境变量指定自定义连接字符串。

`apps/runner` 现在默认以服务器模式启动，开发模式会通过 `cargo watch` 在源码变化后自动重启。

## API

上传 workflow 并定义 workspace：

```bash
curl -i \
  --request POST \
  --url http://127.0.0.1:3002/workflows \
  --header 'content-type: application/json' \
  --data '{
    "workspaceId": "ws-demo",
    "workspaceName": "Demo Workspace",
    "workflow": {
      "meta": {
        "key": "sorting.demo",
        "name": "Sorting Demo",
        "version": 1
      },
      "trigger": { "type": "manual" },
      "inputSchema": { "type": "object" },
      "nodes": [
        { "id": "start_1", "type": "start", "name": "Start" },
        { "id": "end_1", "type": "end", "name": "End" }
      ],
      "transitions": [
        { "from": "start_1", "to": "end_1" }
      ],
      "policies": {}
    }
  }'
```

返回值会包含 `workflowId`，后续执行时使用它。

执行 workflow：

```bash
curl -i \
  --request POST \
  --url http://127.0.0.1:3002/workflows/<workflow_id>/runs \
  --header 'content-type: application/json' \
  --data '{
    "trigger": {
      "headers": { "requestId": "req-api-1" },
      "body": { "orderNo": "SO-API-1", "bizType": "auto_sort" }
    }
  }'
```

返回值会包含 `runId`、`statusUrl`、`eventsUrl`。

查询 workflow 执行状态：

```bash
curl -i \
  --request GET \
  --url http://127.0.0.1:3002/runs/<run_id>
```

使用 SSE 持续订阅运行状态和 timeline 更新：

```bash
curl -N \
  --request GET \
  --url http://127.0.0.1:3002/runs/<run_id>/events
```

恢复 waiting run：

```bash
curl -i \
  --request POST \
  --url http://127.0.0.1:3002/runs/<run_id>/resume \
  --header 'content-type: application/json' \
  --data '{
    "event": {
      "event": "rcs.callback",
      "correlationKey": "req-api-1",
      "status": "done",
      "orderNo": "SO-API-1"
    }
  }'
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
- `code` (`js/javascript`, host Node.js 22+ runtime)
- `sub_workflow`
- `action`
- `command` (`action` alias)
- `wait`
- `task`

## 主要节点使用说明

### 通用约定

- 节点执行时可在 `inputMapping` 中通过模板访问 `trigger / input / state / env`，例如 `{{trigger.body.orderNo}}`、`{{state.orderSnapshot.status}}`。
- 如果节点未配置 `inputMapping`，默认会直接拿上一个节点的 `output` 作为当前节点输入。
- 分支节点会返回 `branchKey`；引擎会优先匹配 transition 的 `label`，其次匹配 `condition`，最后回退到 `branchType: "default"` 或 `label: "default"`。
- `set_state`、`code`、`sub_workflow` 可以生成 `statePatch` 并合并到全局 `state`；后续节点可通过 `{{state.xxx}}` 继续引用。

### `start`

入口节点。默认输出 `trigger.body`；如果没有 `body`，则输出整个 `trigger`。

```json
{ "id": "start_1", "type": "start", "name": "Start" }
```

### `webhook_trigger`

用于从 webhook 触发数据中选择实际载荷。

- `config.mode = "body"`：输出 `trigger.body`，默认值
- `config.mode = "headers"`：输出 `trigger.headers`
- `config.mode = "full"`：输出完整 `trigger`

```json
{
  "id": "webhook_in",
  "type": "webhook_trigger",
  "name": "Webhook Trigger",
  "config": { "mode": "full" }
}
```

### `fetch`

通过 `config.connector` 调用已注册的 connector，节点输出结构固定为 `{ connector, request, data }`。

```json
{
  "id": "fetch_order",
  "type": "fetch",
  "name": "查询订单",
  "config": { "connector": "oms.getOrder" },
  "inputMapping": {
    "orderNo": "{{trigger.body.orderNo}}",
    "warehouseId": "{{env.warehouseId}}"
  }
}
```

### `set_state`

把输入写入运行时状态。`config.path` 指定写入路径；如果 `inputMapping` 中包含 `value` 字段，则取 `value` 作为实际写入值，否则写入整个映射结果。

```json
{
  "id": "persist_order_snapshot",
  "type": "set_state",
  "name": "写入订单快照",
  "config": { "path": "orderSnapshot" },
  "inputMapping": {
    "value": "{{input}}"
  }
}
```

### `if_else`

布尔分支节点。`config.expression` 解析后按 truthy/falsey 判断；如果配置了 `config.equals`，则改为做相等比较。节点返回的 `branchKey` 固定是 `then` 或 `else`，所以 transition 通常这样写：

```json
{
  "id": "dispatch_gate",
  "type": "if_else",
  "name": "Need Dispatch",
  "config": { "expression": "{{trigger.body.needsDispatch}}" }
}
```

```json
[
  { "from": "dispatch_gate", "to": "dispatch_command", "label": "then" },
  { "from": "dispatch_gate", "to": "mark_skipped", "label": "else" }
]
```

### `switch`

多路分支节点。`config.expression` 解析后的字符串值就是 `branchKey`；如果表达式结果是 `null`，则会落到 `default`。

```json
{
  "id": "route_switch",
  "type": "switch",
  "name": "业务分流",
  "config": { "expression": "{{trigger.body.bizType}}" }
}
```

```json
[
  { "from": "route_switch", "to": "manual_review_task", "label": "manual_review" },
  { "from": "route_switch", "to": "dispatch_rcs_action", "label": "auto_sort" },
  { "from": "route_switch", "to": "wait_dispatch_callback", "branchType": "default" }
]
```

### `action` / `command`

`command` 是 `action` 的别名。节点会根据 `config.action` 调用已注册的 action handler，输出 `{ action, response }`。

```json
{
  "id": "dispatch_rcs_action",
  "type": "action",
  "name": "下发 RCS 调度",
  "config": { "action": "rcs.dispatch" },
  "inputMapping": {
    "orderNo": "{{trigger.body.orderNo}}",
    "bizType": "{{trigger.body.bizType}}"
  }
}
```

### `code`

运行 JavaScript 节点，要求宿主环境为 `Node.js 22+`。

- `config.language` / `config.lang` 目前只支持 `js` 或 `javascript`
- 可通过 `config.source` / `js` / `code` 写内联脚本
- 也可通过 `config.sourcePath` / `filePath` 读取文件
- 或通过 `config.modulePath` + `config.exportName` 调用模块导出函数
- 脚本上下文固定为 `trigger / input / state / env / params`，其中 `params` 来自 `inputMapping`
- 返回普通 JSON 时会直接作为节点输出；返回 `{ output, statePatch, branchKey }` 时可同时控制输出、状态更新和分支
- `timeoutMs` 可限制最长执行时间

```json
{
  "id": "run_code",
  "type": "code",
  "name": "Run Code",
  "inputMapping": {
    "orderNo": "{{input.orderNo}}",
    "requestId": "{{trigger.headers.requestId}}"
  },
  "config": {
    "language": "js",
    "modulePath": "examples/code-flow-handler.mjs",
    "exportName": "default"
  },
  "timeoutMs": 3000
}
```

### `respond`

生成一个 `webhook_response` signal，常用于 HTTP/SSE 场景下回包。

- `config.statusCode`：响应状态码，默认 `200`
- `config.terminal = true`：发送响应后直接结束流程

```json
{
  "id": "respond_ok",
  "type": "respond",
  "name": "Respond",
  "config": {
    "statusCode": 202,
    "terminal": false
  },
  "inputMapping": {
    "orderNo": "{{trigger.body.orderNo}}",
    "subWorkflowStatus": "{{state.subWorkflow.status}}"
  }
}
```

### `wait`

把当前 run 挂起为 `waiting`，等待外部事件恢复。

- `config.event` 指定期望事件名，默认 `external_callback`
- `inputMapping` 的结果会作为 waiting signal 的 `payload`
- 恢复时 `/runs/<run_id>/resume` 传入的事件必须匹配 `event/type`
- 如果 waiting payload 中带了 `correlationKey`，恢复事件也必须带相同值

```json
{
  "id": "wait_dispatch_callback",
  "type": "wait",
  "name": "等待回调",
  "config": { "event": "rcs.callback" },
  "inputMapping": {
    "correlationKey": "{{trigger.headers.requestId}}",
    "orderNo": "{{trigger.body.orderNo}}"
  }
}
```

### `task`

通过 `config.taskType` 创建任务并进入 `waiting`。节点会调用已注册的 task handler，输出 `{ taskType, task }`，同时发出 `task_created` signal。

- `config.taskType`：任务类型
- `config.completeEvent`：恢复时期望的完成事件，默认 `task.completed`
- 如果任务创建结果中包含 `taskId`，恢复时也必须带同一个 `taskId`

```json
{
  "id": "manual_review_task",
  "type": "task",
  "name": "创建人工复核任务",
  "config": {
    "taskType": "manual_review",
    "completeEvent": "task.completed"
  },
  "inputMapping": {
    "orderNo": "{{trigger.body.orderNo}}",
    "reason": "bizType requires manual review"
  }
}
```

恢复事件示例：

```json
{
  "event": "task.completed",
  "taskId": "task-run-demo",
  "status": "approved",
  "operatorId": "reviewer-1"
}
```

### `sub_workflow`

执行子流程，既支持内联定义，也支持按 key 引用已注册工作流。

- `config.definition` / `config.workflow`：直接内联子流程定义
- `config.ref` / `config.workflowKey`：引用 registry 中的工作流
- `config.statePath`：把子流程摘要写入指定状态路径
- 子流程完成时继续向下执行；子流程进入 `waiting` 时，父流程也会跟着等待，并在恢复时级联 resume

```json
{
  "id": "nested_workflow",
  "type": "sub_workflow",
  "name": "Nested Workflow",
  "config": {
    "statePath": "nested",
    "ref": "child-wait-flow"
  },
  "inputMapping": {
    "orderNo": "{{trigger.body.orderNo}}"
  }
}
```

### `end`

结束节点。它会把当前 `input` 作为最终输出并终止执行。

```json
{ "id": "end_1", "type": "end", "name": "End" }
```

当前的 `fetch`、`action`、`wait`、`task` 仍是受控 stub，用于先把定义层、状态模型和节点协议跑通。
其中 `fetch / action / task` 已经改为通过 registry 分发，后续接真实外部系统时只需要替换对应 handler。
当前 `code` 节点直接调用宿主 `Node.js 22+` 运行，脚本上下文暴露 `trigger / input / state / env / params` 五个 JSON 对象，其中 `params` 来自节点 `inputMapping`。节点既支持内联 `config.source/js/code`，也支持 `config.sourcePath/filePath` 读取脚本文件，以及 `config.modulePath` 调用外部模块导出的函数；模块模式还支持 `config.exportName` 选择命名导出。返回值可直接作为节点输出，或使用 `{ output, statePatch, branchKey }` 控制状态合并与分支。节点 `timeoutMs` 会限制脚本最长运行时间，`console.log/info/warn/error/debug` 会被捕获并写入 timeline。相对路径默认按 runner 进程当前工作目录解析，也可以通过 `config.baseDir` / `config.workingDirectory` 显式指定基准目录。
当前 `run store` 使用 PostgreSQL 持久化存储，支持工作流运行摘要和快照的持久化。引擎与持久化边界已拆开，可通过实现 `WorkflowRunStore` trait 替换为其他存储实现（如 MySQL / Redis / KV 存储）。
当前 `sub_workflow` 支持同步执行，也支持子流程进入 `waiting` 后由父流程代理等待并级联恢复。
