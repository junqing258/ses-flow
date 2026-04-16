# 节点说明

## 节点类型总览

runner 当前支持这些节点类型：

- `start`
- `end`
- `fetch`
- `set_state`
- `switch`
- `shell`
- `wait`
- `task`
- `respond`
- `code`
- `sub_workflow`
- `webhook_trigger`
- `if_else`

其中当前前端画布和 AI 预览已经较好对齐的常见节点是：

- `start`
- `end`
- `fetch`
- `switch`
- `shell`
- `wait`
- `task`
- `respond`
- `code`
- `sub_workflow`
- `webhook_trigger`
- `if_else`

`set_state` 目前是 runner 支持但前端编辑器未完整对齐的类型。若任务目标是可视化 AI 预览，除非用户明确要求，否则不要优先引入它。

## `start`

- 用途：工作流唯一入口节点。
- 必要字段：`id`、`type: "start"`、`name`。
- 规则：一个工作流必须且只能有一个 `start` 节点。
- 常见连线：通常只有一条出边，不需要复杂 `config`。

## `end`

- 用途：结束流程。
- 必要字段：`id`、`type: "end"`、`name`。
- 规则：可有多个 `end` 节点，用于表达不同结束状态。
- 常见做法：通过不同终点表达成功结束、人工处理、等待回调等结果。

## `webhook_trigger`

- 用途：表示画布中的 webhook 入口节点。
- 常见字段：
  - `type: "webhook_trigger"`
  - `config.mode`，当前前端默认写 `"body"`
  - `annotations.editorPosition`
- 注意：真正的触发方式仍以顶层 `workflow.trigger` 为准；当前前端会同时生成：
  - 顶层 `trigger.type = "webhook"`
  - 节点 `type = "webhook_trigger"`
- 常见编辑项：
  - `trigger.path`
  - `trigger.responseMode`
  - 节点面板中的 `payload` 映射

## `fetch`

- 用途：发起 HTTP 请求获取外部数据。
- 关键字段：
  - `config.method`
  - `config.url`
  - `config.headers`
  - `inputMapping`
  - 可选 `outputMapping`
- 典型场景：查询订单、调用外部服务、拉取配置。
- 建议：URL、headers 放 `config`，请求体或查询参数放 `inputMapping`。

## `switch`

- 用途：根据表达式结果做多路分流。
- 关键字段：
  - `config.expression`
  - `annotations.switchBranches`
  - `annotations.defaultBranchHandle`
- 连线规则：
  - 分支线通常通过 `transitions[].label` 标识分支标签。
  - 默认分支通过 `transitions[].branchType = "default"` 表示。
- 说明：为了让前端画布正确恢复分支手柄，AI 编辑时应同时维护 `annotations.switchBranches` 和默认分支信息。

## `if_else`

- 用途：布尔条件分支，是双分支特化版控制节点。
- 关键字段：
  - `config.expression`
- 连线规则：
  - `then` 分支通常使用 `label: "then"`
  - `else` 分支通常使用 `label: "else"`
- 适用场景：只需要真假两路判断时，比通用 `switch` 更直接。

## `shell`

- 用途：执行 shell 命令。
- 关键字段：
  - `config.command`
  - 可选 `config.shell`
  - 可选 `config.workingDirectory`
  - `inputMapping`
- 说明：前端默认将 `inputMapping` 作为 JSON 注入标准输入，并提供 `WORKFLOW_PARAMS` 环境变量语义说明。

## `code`

- 用途：执行内联 JavaScript 或 TypeScript 逻辑。
- 关键字段：
  - `config.language`
  - `config.source`
  - `inputMapping`
- 常见习惯：把 `inputMapping` 映射成 `params` 语义，再在代码中读取 `trigger`、`input`、`state`、`env`。

## `wait`

- 用途：等待事件、回调或异步恢复。
- 关键字段：
  - `config.event`
  - 可选 `timeoutMs`
- 常见场景：等待设备回调、人工确认、外部任务完成通知。

## `task`

- 用途：创建任务并等待外部完成事件。
- 关键字段：
  - `config.taskType`
  - `config.completeEvent`
  - `inputMapping`
- 当前前端默认：
  - `taskType` 来自面板中的 `command`
  - `completeEvent` 固定为 `"task.completed"`

## `respond`

- 用途：给同步 webhook 请求返回结果。
- 关键字段：
  - `config.statusCode`
  - `inputMapping`
- 建议：当 `workflow.trigger.responseMode = "sync"` 时，通常应规划 `respond` 节点来输出响应内容。

## `sub_workflow`

- 用途：调用子工作流。
- 关键字段：
  - `config.workflowKey`
  - `inputMapping`
- 建议：保留主流程与子流程边界清晰，不要把子流程 key 和当前工作流 key 混用。

## `set_state`

- 用途：直接写入或覆盖运行时状态。
- 当前状态：runner 支持，但当前前端调色板、导入和 AI 预览没有完整对齐。
- 建议：如果只是维护 AI 预览草稿，尽量避免直接新增该类型，除非同时确认前端兼容策略。
