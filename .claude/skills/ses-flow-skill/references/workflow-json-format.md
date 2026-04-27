# Workflow JSON 数据格式

## 适用范围

这份文档描述 SES Flow 在 AI 编辑会话中最常用的三类 JSON 结构：

- 编辑会话请求体
- runner `workflow`
- 前端 `editorDocument`

## 1. 编辑会话请求体

`POST /edit-sessions` 和 `PUT /edit-sessions/{session_id}/draft` 使用同一份请求体结构：

```json
{
  "workspaceId": "ses-workflow-editor",
  "workflowId": "wf-optional",
  "editorDocument": {},
  "workflow": {}
}
```

- `workspaceId`
  首次创建会话时建议显式提供。更新时通常可以省略。
- `workflowId`
  如果该 AI 草稿绑定到某个已存在工作流，应该保留它；新草稿可以省略。
- `editorDocument`
  技术上可选，但 AI 预览场景建议始终一并发送，否则 Web 只读画布可能无法恢复布局、面板和选中状态。
- `workflow`
  必填，且必须是完整的 runner 工作流定义，不能只传局部节点补丁。

## 2. `workflow` 顶层结构

推荐骨架如下：

```json
{
  "meta": {
    "key": "sorting-main-flow",
    "name": "Sorting Main Flow",
    "version": 3,
    "status": "draft"
  },
  "trigger": {
    "type": "webhook",
    "path": "/api/workflow/inbound-order",
    "responseMode": "async_ack"
  },
  "inputSchema": {
    "type": "object"
  },
  "nodes": [],
  "transitions": [],
  "policies": {
    "allowManualRetry": true
  }
}
```

- `meta.key`
  工作流稳定标识；AI 编辑草稿时应尽量保留，不要随意改名。
- `meta.name`
  展示名称；通常可改。
- `meta.version`
  正整数；如果只是草稿编辑，通常保持现有版本。
- `meta.status`
  常见值是 `draft` 或 `published`；AI 草稿通常保持 `draft`。
- `trigger.type`
  当前 runner 支持 `manual`、`webhook`、`event`。
- `trigger.path`
  仅 `webhook` 触发器需要。
- `trigger.responseMode`
  仅 `webhook` 触发器常用，支持 `sync` 和 `async_ack`。
- `inputSchema`
  当前前端默认使用 `{ "type": "object" }`。
- `nodes`
  节点数组，每个节点至少包含 `id`、`type`、`name`，其余字段按节点类型决定。
- `transitions`
  连线数组，每项至少包含 `from` 和 `to`。
- `policies`
  当前常见字段为 `allowManualRetry`；runner 还支持工作流级别的 `timeout_ms`、`retry_policy`、`idempotency`、`audit_level`、`data_retention`。

## 3. `workflow.nodes[]` 通用字段

```json
{
  "id": "fetch_order",
  "type": "fetch",
  "name": "查询订单",
  "config": {},
  "inputMapping": {},
  "outputMapping": {},
  "timeoutMs": 3000,
  "retryPolicy": {
    "max_attempts": 2,
    "backoff_ms": 500
  },
  "onError": {
    "strategy": "fail_fast",
    "nextNodeId": "notify_failure"
  },
  "annotations": {
    "editorPosition": {
      "x": 400,
      "y": 180
    },
    "note": "供编辑器展示"
  }
}
```

- `id`
  节点唯一标识。AI 编辑时通常应保留已有 id，避免破坏连线和面板映射。
- `type`
  runner 节点类型字符串；详细语义见 [node-reference.md](node-reference.md)。
- `name`
  节点显示名称。
- `config`
  节点私有配置。
- `inputMapping`
  传给节点执行器的输入。可为对象、字符串或 `null`。
- `outputMapping`
  节点输出写回状态时使用；当前前端主要在 `fetch` 导入时保留此字段。
- `timeoutMs`
  节点级超时时间。
- `retryPolicy`
  节点级重试策略，runner 字段名是 `max_attempts` / `backoff_ms`。
- `onError`
  节点错误处理策略。
- `annotations`
  非运行时核心字段，常用于编辑器位置、备注和分支元数据。

## 4. `transitions` 结构

```json
{
  "from": "switch_biz_type",
  "to": "wait_callback",
  "label": "B",
  "priority": 90,
  "branchType": "default"
}
```

- `from` / `to`
  必须引用已存在节点 id。
- `label`
  分支标签。`switch` 和 `if_else` 常用。
- `priority`
  数值越大越优先。
- `branchType`
  当前主要使用 `default`，表示默认分支。

## 5. `editorDocument` 结构

前端持久化结构如下：

```json
{
  "schemaVersion": "1.0",
  "editor": {
    "activeTab": "base",
    "pageMode": "ai",
    "selectedNodeId": "fetch_order",
    "runDraft": {
      "body": "{}",
      "env": "{}",
      "headers": "{\n  \"x-request-id\": \"wf-run-demo-001\"\n}",
      "triggerMode": "manual"
    }
  },
  "graph": {
    "nodes": [],
    "edges": [],
    "panels": {}
  },
  "workflow": {
    "id": "sorting-main-flow",
    "name": "Sorting Main Flow",
    "status": "draft",
    "version": "v3"
  }
}
```

- `schemaVersion`
  当前固定为 `"1.0"`。
- `editor.pageMode`
  AI 预览时应设为 `"ai"`；普通编辑页通常是 `"edit"`，运行态为 `"run"`。
- `editor.activeTab`
  当前右侧面板激活标签，常见值为 `base`、`mapping`、`retry`、`error`。
- `editor.selectedNodeId`
  当前选中节点 id。
- `editor.runDraft`
  运行页草稿，请保留已有结构，哪怕 AI 模式当前不直接使用。
- `graph.nodes`
  画布节点数组。**必须包含完整的 `data` 对象**，否则前端会抛 `Cannot read properties of undefined (reading 'kind')`。格式详见下方"graph.nodes 节点格式"章节。
- `graph.edges`
  画布连线数组，边的 `source` / `target` 应与节点 id 对齐；`if_else` / `switch` 分支边需要加 `sourceHandle`（值与 transition `label` 一致）。
- `graph.panels`
  右侧配置面板数据，key 是节点 id；AI 编辑时如果变更了节点配置，最好同步更新这里。
- `workflow`
  前端展示用的工作流元数据快照，不替代 runner 的 `workflow.meta`。

## 5b. graph.nodes 节点格式

`graph.nodes` 中每个元素是 React Flow 节点，**必须包含 `data` 字段**。缺少 `data` 会导致前端崩溃。

### 通用结构

```json
{
  "id": "节点id",
  "type": "terminal | workflow-card",
  "position": { "x": 0, "y": 0 },
  "sourcePosition": "right",
  "targetPosition": "left",
  "data": {
    "kind": "...",
    "accent": "#rrggbb",
    "icon": "...",
    "nodeKey": "节点id",
    "title": "显示标题",
    "subtitle": "副标题（可选）",
    "runnerType": "plugin:xxx（plugin 节点必填）"
  }
}
```

- `type` 字段：`start` / `end` 节点用 `"terminal"`，其余全部用 `"workflow-card"`
- `data.nodeKey` 必须与 `id` 相同

### 各节点类型速查表

| runner type | data.kind | data.accent | data.icon | React Flow type |
|---|---|---|---|---|
| `start` | `"start"` | `"#10B981"` | `"play"` | `"terminal"` |
| `end` | `"end"` | `"#EF4444"` | `"shield"` | `"terminal"` |
| `wait` | `"wait"` | `"#F59E0B"` | `"clock"` | `"workflow-card"` |
| `if_else` | `"if-else"` | `"#F97316"` | `"gitBranch"` | `"workflow-card"` |
| `switch` | `"switch"` | `"#EC4899"` | `"gitBranch"` | `"workflow-card"` |
| `fetch` | `"fetch"` | `"#3B82F6"` | `"database"` | `"workflow-card"` |
| `code` | `"effect"` | `"#0F766E"` | `"code"` | `"workflow-card"` |
| `shell` | `"shell"` | `"#F97316"` | `"zap"` | `"workflow-card"` |
| `sub_workflow` | `"sub-workflow"` | `"#6366F1"` | `"webhook"` | `"workflow-card"` |
| `webhook_trigger` | `"trigger"` | `"#6366F1"` | `"webhook"` | `"workflow-card"` |
| `plugin:*`（未注册） | `"effect"` | `"#8B5CF6"` | `"activity"` | `"workflow-card"` |
| `plugin:scan_task` | `"effect"` | `"#F97316"` | `"scan-barcode"` | `"workflow-card"` |
| `plugin:get_task_info` | `"effect"` | `"#6366F1"` | `"clipboard-list"` | `"workflow-card"` |
| `plugin:robot_departure` | `"effect"` | `"#10B981"` | `"truck"` | `"workflow-card"` |
| `plugin:pack_task` | `"effect"` | `"#14B8A6"` | `"badge-check"` | `"workflow-card"` |

> 已注册插件的 `accent` / `icon` 以插件 descriptor 中定义的 `color` / `icon` 字段为准，优先于上表。

### if_else 节点需额外带 branchHandles

```json
"data": {
  "kind": "if-else",
  "branchHandles": [
    { "id": "then", "label": "then", "isDefault": false },
    { "id": "else", "label": "else", "isDefault": true }
  ]
}
```

对应的出边需设置 `"sourceHandle": "then"` / `"sourceHandle": "else"`。

### switch 节点需额外带 branchHandles

每个分支一个条目，`isDefault: true` 标记默认分支：

```json
"data": {
  "kind": "switch",
  "branchHandles": [
    { "id": "branch-a", "label": "A", "isDefault": false },
    { "id": "branch-b", "label": "B", "isDefault": false },
    { "id": "branch-default", "label": "default", "isDefault": true }
  ]
}
```

## 6. 映射与表达式约定

- 优先使用对象形式的 `inputMapping` / `outputMapping`，比自由文本更稳定。
- 前端会把诸如 `payload.orderId`、`body.orderId`、`response.data` 之类简写，归一化为模板引用。
- 常见引用根路径：
  - `trigger.body`
  - `trigger.headers`
  - `input`
  - `state`
  - `env`
- 显式模板建议写成 `{{trigger.body.orderId}}`、`{{input.data}}`、`{{state.someKey}}`。
- 如果你修改了节点 id，也必须同步修复：
  - `workflow.transitions[].from`
  - `workflow.transitions[].to`
  - `editorDocument.graph.nodes[].id`
  - `editorDocument.graph.edges[].source`
  - `editorDocument.graph.edges[].target`
  - `editorDocument.graph.panels` 的 key

## 7. runner 校验重点

- 必须且只能存在一个 `start` 节点。
- 节点 `id` 不能重复。
- 所有连线必须引用存在的节点。
- 历史别名 `action` / `command` 会被 runner 归一化为 `shell`。
- `subworkflow` 会被视为 `sub_workflow`。
- `webhook` 会被视为 `webhook_trigger`。
- `if` 会被视为 `if_else`。
