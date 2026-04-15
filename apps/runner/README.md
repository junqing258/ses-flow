# Runner

`apps/runner` 是仓储/物流分拣作业编排平台的自研 DSL Runner 原型，采用 Rust 实现。

当前版本提供：

- 稳定的 `Workflow Definition JSON` 定义模型
- 最小可运行的执行内核
- 内置基础节点执行器
- `shell / task` 节点执行能力
- workspace + workflow 注册能力
- `run / snapshot` store 抽象与 PostgreSQL 持久化实现
- `waiting -> resume` 恢复执行能力
- `sub_workflow` 父子级联恢复
- `resume` 事件类型与关联键校验
- HTTP API 运行状态查询

## Database Setup

该项目使用 PostgreSQL 作为持久化存储。详细的 PostgreSQL 设置说明请参考 [POSTGRES_SETUP.md](./POSTGRES_SETUP.md)。

### 快速启动（使用 Docker Compose）

```bash

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
# dev安装 cargo-watch cargo install cargo-watch --locked
cargo watch -x 'run -- --host 127.0.0.1 --port 3002'
cargo run -- --host 127.0.0.1 --port 3002

# with custom PostgreSQL database URL
cargo run -- --host 127.0.0.1 --port 3002 --database-url "postgresql://user:password@localhost/dbname"

# or use environment variable
export DATABASE_URL="postgresql://user:password@localhost/dbname"
cargo run -- --host 127.0.0.1 --port 3002

# optionally restrict CORS origins instead of allowing all origins
export RUNNER_CORS_ALLOW_ORIGINS="http://localhost:5173,https://ses.example.com"
cargo run -- --host 127.0.0.1 --port 3002

cargo test
```

默认使用 PostgreSQL 持久化存储，数据库连接字符串默认为 `postgresql://runner:runner@localhost/runner`。可以通过 `--database-url` 参数或 `DATABASE_URL` 环境变量指定自定义连接字符串。

Runner API 默认开启跨域支持，便于前端或本地工具直接访问。若设置 `RUNNER_CORS_ALLOW_ORIGINS`，则会按逗号分隔的 origin 白名单收敛允许的跨域来源；未设置时默认允许所有来源。

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

### 关于 webhook 触发的当前实现

- 当前 runner 还没有按 workflow `trigger.path` 自动注册真实的 HTTP webhook 路由。
- 现在所有 workflow 都是通过统一入口 `POST /workflows/<workflow_id>/run` 启动。
- 请求里的 `trigger` 对象会作为整次运行的触发上下文传入引擎，后续节点再从中读取 `headers / body / ...`。
- 因此，workflow 定义里的 `trigger.type = "webhook"`、`trigger.path`、`trigger.responseMode` 目前主要是定义层元数据，还没有在 API 路由层完成消费。

执行 workflow：

```bash
  curl -i \
  --request POST \
  --url http://127.0.0.1:3002/workflows/<workflow_id>/run \
  --header 'content-type: application/json' \
  --data '{
    "trigger": {
      "headers": { "requestId": "req-api-1" },
      "body": { "orderNo": "SO-API-1", "bizType": "auto_sort" }
    }
  }'
```

返回值会包含 `runId`、`statusUrl`。

查询 workflow 执行状态：

```bash
curl -i \
  --request GET \
  --url http://127.0.0.1:3002/runs/<run_id>
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
- `switch`
- `code` (`js/javascript/ts/typescript`, host Node.js 22.20+ runtime)
- `shell`
- `sub_workflow`
- `shell`
- `wait`
- `task`

## 主要节点使用说明

### 通用约定

- 节点执行时可在 `inputMapping` 中通过模板访问 `trigger / input / state / env`，例如 `{{trigger.body.orderNo}}`、`{{state.orderSnapshot.status}}`。
- 如果节点未配置 `inputMapping`，默认会直接拿上一个节点的 `output` 作为当前节点输入。
- 分支节点会返回 `branchKey`；引擎会优先匹配 transition 的 `label`，其次匹配 `condition`，最后回退到 `branchType: "default"` 或 `label: "default"`。
- `set_state`、`code`、`sub_workflow` 可以生成 `statePatch` 并合并到全局 `state`；后续节点可通过 `{{state.xxx}}` 继续引用。

### 下个节点如何拿上个节点输出

默认规则很简单：上一个节点的 `output` 会成为下一个节点的 `input`。

- 如果下一个节点不写 `inputMapping`，那它拿到的整个 `input` 就是上一个节点的完整输出。
- 如果下一个节点写了 `inputMapping`，就通过 `{{input.xxx}}` 从上一个节点输出里挑字段。

常见写法：

- 上一个节点是 `fetch` 时，可在下个节点里取 `{{input.data}}`、`{{input.response.status}}`、`{{input.request.orderNo}}`
- 上一个节点是 `shell` 时，可在下个节点里取 `{{input.data}}`、`{{input.stdout}}`、`{{input.exitCode}}`
- 上一个节点是 `code` 时，可在下个节点里取 `{{input}}`，或者取 `{{input.xxx}}`；如果 code 返回了 `{ output, statePatch }`，这里的 `input` 指的是其中的 `output`

示例：让 `set_state` 保存上一个节点结果的一部分

```json
{
  "id": "persist_result",
  "type": "set_state",
  "name": "保存结果",
  "config": { "path": "result" },
  "inputMapping": {
    "value": {
      "status": "{{input.response.status}}",
      "payload": "{{input.data}}"
    }
  }
}
```

### `start`

入口节点。默认输出 `trigger.body`；如果没有 `body`，则输出整个 `trigger`。

```json
{ "id": "start_1", "type": "start", "name": "Start" }
```

### `webhook_trigger`

用于从 webhook 触发数据中选择实际载荷。

需要特别说明的是：`webhook_trigger` 当前是“流程内的触发数据选择节点”，不是“自动暴露 HTTP webhook endpoint 的路由节点”。

- runner 当前不会根据 workflow 的 `trigger.path` 自动生成类似 `/webhooks/...` 的 HTTP 入口。
- 实际触发方式仍然是调用 `POST /workflows/<workflow_id>/run`，并在请求体中传入 `trigger`。
- `trigger.type = "webhook"`、`trigger.path`、`trigger.responseMode` 当前主要用于表达 workflow 的触发意图与元数据。
- `respond` 节点会产出 `webhook_response` signal，但当前 API 层还不会把它自动回写成某个真实 webhook 请求的同步 HTTP 响应。
- 当 `trigger.type = "webhook"` 且 `trigger.responseMode = "sync"` 时，如果流程没有显式 `respond` 节点、而是直接运行到 `end`，runner 会在完成时自动补一个默认的 `webhook_response` signal，等价于返回 `200 + 最终输出 body`。

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

按 `config.method` + `config.url` 直接发起 HTTP 请求，支持 `GET` 和 `POST`。

- `GET`：会把 `inputMapping` 解析结果作为 query string
- `POST`：会把 `inputMapping` 解析结果作为 JSON body
- 可选的 `config.headers` 会作为请求头发送
- 节点输出结构固定为 `{ method, url, request, response, data }`
- 下个节点通常通过 `{{input.data}}` 取响应体，通过 `{{input.response.status}}` 取状态码

```json
{
  "id": "fetch_order",
  "type": "fetch",
  "name": "查询订单",
  "config": {
    "method": "GET",
    "url": "https://jsonplaceholder.typicode.com/todos",
    "headers": {
      "x-source": "runner-demo"
    }
  },
  "inputMapping": {
    "userId": "{{trigger.body.userId}}"
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

### `shell`

`shell` 节点会通过本机 shell 执行 `config.command`，并把 `inputMapping` 结果：

- 写入 stdin（JSON）
- 注入 `WORKFLOW_PARAMS` / `WORKFLOW_TRIGGER` / `WORKFLOW_INPUT` / `WORKFLOW_STATE` / `WORKFLOW_ENV` 等环境变量

节点输出结构为 `{ shell, command, exitCode, stdout, stderr, data }`，其中 `data` 会优先按 JSON 解析 stdout。
- 下个节点通常优先取 `{{input.data}}`；如果 shell 输出的不是 JSON，也可以直接取 `{{input.stdout}}`

```json
{
  "id": "dispatch_rcs_action",
  "type": "shell",
  "name": "下发 RCS 调度",
  "config": {
    "command": "printf '%s' \"$WORKFLOW_PARAMS\"",
    "shell": "sh"
  },
  "inputMapping": {
    "orderNo": "{{trigger.body.orderNo}}",
    "bizType": "{{trigger.body.bizType}}"
  }
}
```

### `code`

运行 JavaScript / TypeScript 节点，要求宿主环境为 `Node.js 22.20+`。

- `config.language` / `config.lang` 目前支持 `js` / `javascript` / `ts` / `typescript`
- 可通过 `config.source` / `js` / `code` 写内联脚本
- 也可通过 `config.sourcePath` / `filePath` 读取文件
- 或通过 `config.modulePath` + `config.exportName` 调用模块导出函数
- 脚本上下文固定为 `trigger / input / state / env / params`，其中 `params` 来自 `inputMapping`
- 返回普通 JSON 时会直接作为节点输出；返回 `{ output, statePatch, branchKey }` 时可同时控制输出、状态更新和分支
- `timeoutMs` 可限制最长执行时间
- 下个节点通过 `{{input}}` 或 `{{input.xxx}}` 读取 code 的返回值；如果用了 envelope 返回，则读取的是 `output`
- `typescript` 会使用 Node.js 的类型转换能力执行；内联脚本和 `.ts/.mts` 模块都要求宿主版本满足 `22.20+`

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
    "language": "typescript",
    "modulePath": "examples/code-flow-handler.mjs",
    "exportName": "default"
  },
  "timeoutMs": 3000
}
```

### `respond`

生成一个 `webhook_response` signal，常用于 HTTP 场景下回包。

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

当前的 `wait`、`task` 仍是受控 stub，用于先把定义层、状态模型和节点协议跑通。
其中 `fetch` 已支持直接发起真实 HTTP 请求；`shell` 已支持直接执行本机命令。
当前 `code` 节点直接调用宿主 `Node.js 22+` 运行，脚本上下文暴露 `trigger / input / state / env / params` 五个 JSON 对象，其中 `params` 来自节点 `inputMapping`。节点既支持内联 `config.source/js/code`，也支持 `config.sourcePath/filePath` 读取脚本文件，以及 `config.modulePath` 调用外部模块导出的函数；模块模式还支持 `config.exportName` 选择命名导出。返回值可直接作为节点输出，或使用 `{ output, statePatch, branchKey }` 控制状态合并与分支。节点 `timeoutMs` 会限制脚本最长运行时间，`console.log/info/warn/error/debug` 会被捕获并写入 timeline。相对路径默认按 runner 进程当前工作目录解析，也可以通过 `config.baseDir` / `config.workingDirectory` 显式指定基准目录。
当前 `run store` 使用 PostgreSQL 持久化存储，支持工作流运行摘要和快照的持久化。引擎与持久化边界已拆开，可通过实现 `WorkflowRunStore` trait 替换为其他存储实现（如 MySQL / Redis / KV 存储）。
当前 `sub_workflow` 支持同步执行，也支持子流程进入 `waiting` 后由父流程代理等待并级联恢复。
