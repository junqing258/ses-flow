# HTTP 插件 Descriptor 约定

## 适用范围

这份文档描述 SES Flow HTTP 插件对外暴露的 descriptor 格式，以及 AI 编辑工作流时应遵守的插件节点约束。

HTTP 插件通常通过以下端点被 runner 注册：

- `GET {plugin_base_url}/descriptors`
- 兼容旧接口：`GET {plugin_base_url}/descriptor`
- 可选元数据接口：`GET {plugin_base_url}/health`

注册成功后，前端会通过 runner 的 `/node-descriptors` 获取 descriptor，并按 descriptor 生成插件调色板和配置面板。

## Descriptor 骨架

```json
{
  "id": "barcode_scan_wait",
  "kind": "wait",
  "runnerType": "plugin:barcode_scan_wait",
  "version": "1.0.0",
  "category": "等待 / 异步",
  "displayName": "等待扫码",
  "description": "创建扫码任务并等待扫码完成回调",
  "color": "#F59E0B",
  "icon": "clock",
  "status": "stable",
  "requiredPermissions": [],
  "transport": "http",
  "timeoutMs": 5000,
  "supportsCancel": false,
  "supportsResume": true,
  "configSchema": {
    "type": "object",
    "properties": {
      "event": {
        "type": "string",
        "title": "等待事件",
        "default": "barcode.scan.completed"
      }
    }
  },
  "defaults": {
    "event": "barcode.scan.completed"
  },
  "inputMappingSchema": {
    "type": "object"
  },
  "outputMappingSchema": {
    "type": "object"
  }
}
```

## 字段说明

- `id`
  插件 descriptor 稳定 id。建议使用小写 snake_case，不要包含空格。
- `kind`
  给前端选择图标、节点风格和分类语义使用。可以是已有节点语义，如 `effect`、`wait`、`fetch`，也可以是前端可兼容的业务分类。等待型插件建议写 `wait`，但这不等于 runner 内置 `wait` 节点。
- `runnerType`
  runner 实际写入 `workflow.nodes[].type` 的值。HTTP 插件必须以 `plugin:` 开头，例如 `plugin:barcode_scan_wait`。
- `version`
  descriptor 版本。建议使用语义化版本或明确可比较的版本字符串。
- `category`
  前端调色板分组名。插件应用下多个节点建议使用一致分类。
- `displayName`
  前端展示名，不能为空。
- `description`
  节点说明，用于面板备注或帮助信息。
- `color`
  前端节点强调色，建议使用十六进制颜色，如 `#F59E0B`。
- `icon`
  前端图标 key。优先使用项目已有 lucide 图标 key，如 `clock`、`activity`、`scan-barcode`、`clipboard-list`、`truck`。
- `status`
  支持 `stable`、`beta`、`deprecated`。前端通常隐藏或弱化 `deprecated` 节点。
- `requiredPermissions`
  插件声明的权限列表。当前主要用于展示和未来权限校验。
- `transport`
  当前 HTTP 插件必须是 `"http"`。非 HTTP transport 目前不要用于插件自动注册。
- `timeoutMs`
  runner 调用插件 `/execute` 的默认超时时间；节点级 `timeoutMs` 可覆盖它。
- `supportsCancel`
  是否声明支持取消。当前主要是能力声明。
- `supportsResume`
  是否声明支持恢复。等待型插件应设为 `true`。
- `configSchema`
  生成前端配置面板的 JSON schema。`properties` 中每个字段会转成面板配置项，并在导出 workflow 时写入节点 `config`。
- `defaults`
  配置默认值。若 `configSchema.properties[*].default` 与 `defaults` 都存在，前端会优先使用 `defaults` 中对应字段。
- `inputMappingSchema`
  输入映射提示。当前前端主要保留通用 `payload` 映射，schema 可作为插件契约说明。
- `outputMappingSchema`
  输出映射提示。用于说明插件成功或恢复后的输出结构。

runner 注册后可能补充这些字段：

- `endpoint`
  插件 base URL，由 runner 注册时写入，不要求插件自己返回。
- `pluginAppId` / `pluginAppName`
  runner 会尝试从 `/health` 的 `appId`、`appName`、`pluginId`、`pluginName`、`displayName` 中补齐。

## 硬性约束

- `runnerType` 必须以 `plugin:` 开头。
- `transport` 必须是 `"http"`。
- `displayName` 不能为空。
- `/descriptors` 可以返回 descriptor 数组，也可以返回单个 descriptor 对象；空数组会被视为错误。
- 如果 `/descriptors` 返回 404，runner 会降级请求 `/descriptor`。
- 同一个 `runnerType` 应只对应一个当前可用 descriptor，避免前端和 runner 解析到不同节点。
- descriptor 不应返回 `endpoint` 作为事实来源；endpoint 以注册时的 base URL 为准。
- `configSchema` 必须保持 JSON object 结构，至少包含 `type: "object"` 与 `properties`。
- `configSchema.properties` 的 key 应稳定；修改 key 会影响已保存工作流的 `config`。
- 需要等待恢复的插件应设置 `supportsResume: true`，并在 `/execute` 返回 `status: "waiting"` 时提供 `waitSignal`。

## 前端生成规则

已注册插件会进入前端动态调色板：

- 节点 `data.runnerType` 来自 descriptor 的 `runnerType`。
- 前端导出 runner workflow 时，若 `data.runnerType` 存在，会优先把它写成 `workflow.nodes[].type`。
- 插件节点配置面板会根据 `configSchema.properties` 生成 `config:*` 字段。
- 导出时，`config:*` 字段会去掉前缀并写入节点 `config`。
- 插件节点的 `inputMapping` 来自面板中的通用 `payload` 映射。

插件节点在 `workflow.nodes[]` 中的典型形态：

```json
{
  "id": "barcode_scan_wait_1",
  "type": "plugin:barcode_scan_wait",
  "name": "等待扫码",
  "config": {
    "event": "barcode.scan.completed"
  },
  "inputMapping": {
    "requestId": "{{trigger.headers.requestId}}",
    "orderNo": "{{trigger.body.orderNo}}"
  },
  "timeoutMs": 5000
}
```

对应 `editorDocument.graph.nodes[]` 必须保留 `runnerType`：

```json
{
  "id": "barcode_scan_wait_1",
  "type": "workflow-card",
  "position": { "x": 480, "y": 180 },
  "sourcePosition": "right",
  "targetPosition": "left",
  "data": {
    "kind": "wait",
    "accent": "#F59E0B",
    "icon": "clock",
    "nodeKey": "barcode_scan_wait_1",
    "runnerType": "plugin:barcode_scan_wait",
    "title": "等待扫码",
    "subtitle": "创建扫码任务并等待回调"
  }
}
```

## HTTP 插件执行请求

runner 调用插件：

```http
POST {plugin_base_url}/execute
Content-Type: application/json
```

请求体骨架：

```json
{
  "pluginId": "barcode_scan_wait",
  "runnerType": "plugin:barcode_scan_wait",
  "nodeId": "barcode_scan_wait_1",
  "config": {
    "event": "barcode.scan.completed"
  },
  "context": {
    "runId": "run-123",
    "requestId": "req-001",
    "traceId": "trace-001",
    "workflowKey": "sorting-main-flow",
    "workflowVersion": 3,
    "input": {},
    "state": {},
    "env": {
      "tenantId": "tenant-a",
      "warehouseId": "WH-1",
      "operatorId": "system",
      "sesBaseUrl": "http://localhost:6302/runner-api"
    }
  }
}
```

说明：

- `config` 是节点配置经过模板解析后的结果。
- `context.input` 是节点 `inputMapping` 解析后的结果。
- `context.requestId` 会优先从触发器、节点输入或状态中的 `requestId` / `x-request-id` 提取；没有则使用 `runId`。
- `context.traceId` 会优先从触发器、节点输入或状态中的 `traceId` / `x-trace-id` 提取。

## 插件响应格式

成功完成：

```json
{
  "status": "success",
  "output": {
    "barcode": "ABC123"
  },
  "statePatch": {
    "barcodeScan": {
      "status": "completed"
    }
  },
  "logs": [
    {
      "level": "info",
      "message": "barcode scan completed",
      "fields": {
        "barcode": "ABC123"
      }
    }
  ]
}
```

进入等待：

```json
{
  "status": "waiting",
  "output": {
    "taskId": "task-001",
    "requestId": "req-001"
  },
  "statePatch": {
    "barcodeScan": {
      "status": "waiting",
      "taskId": "task-001"
    }
  },
  "waitSignal": {
    "type": "barcode.scan.completed",
    "payload": {
      "requestId": "req-001",
      "taskId": "task-001"
    }
  },
  "logs": [
    {
      "level": "info",
      "message": "barcode scan task created",
      "fields": {
        "taskId": "task-001"
      }
    }
  ]
}
```

失败：

```json
{
  "status": "failed",
  "error": {
    "code": "barcode_scan_failed",
    "message": "扫码任务创建失败",
    "retryable": false
  },
  "logs": [
    {
      "level": "error",
      "message": "failed to create barcode scan task",
      "fields": {}
    }
  ]
}
```

约束：

- `status` 只使用 `success`、`waiting`、`failed`。
- `status: "waiting"` 时必须提供 `waitSignal`。
- `waitSignal` 也兼容字段名 `nextSignal`，但新插件建议统一使用 `waitSignal`。
- `status: "failed"` 时必须提供 `error`。
- `statePatch` 可省略或为 `null`；需要写入运行状态时使用 object。
- `logs[].level` 建议使用 `info`、`warn`、`error`。

## 等待型插件恢复

等待型 HTTP 插件不继承内置 `wait` 节点。它的等待语义由插件响应实现：

1. `/execute` 返回 `status: "waiting"`。
2. 响应中包含 `waitSignal.type` 和 `waitSignal.payload`。
3. runner 保存当前节点为等待节点，并记录 `lastSignal`。
4. 外部系统调用 workflow resume 接口。

恢复请求：

```http
POST /runner-api/runs/{runId}/resume
Content-Type: application/json
```

```json
{
  "event": {
    "type": "barcode.scan.completed",
    "payload": {
      "status": "success",
      "output": {
        "barcode": "ABC123",
        "taskId": "task-001"
      },
      "statePatch": {
        "barcodeScan": {
          "status": "completed",
          "barcode": "ABC123"
        }
      }
    }
  }
}
```

恢复校验规则：

- 回调中的 `event.type` 或 `event.event` 必须等于等待时的 `waitSignal.type`。
- 插件节点当前不自动校验 `correlationKey` / `requestId` 是否一致；插件或回调入口应自行防止串单。
- 若 `payload.status` 缺省且没有 `payload.error`，runner 会按成功恢复处理。
- 成功恢复的输出来自 `payload.output`；若没有 `output`，则使用整个 `payload`。
- 失败恢复使用 `payload.status: "failed"` 与 `payload.error`。

失败恢复示例：

```json
{
  "event": {
    "type": "barcode.scan.completed",
    "payload": {
      "status": "failed",
      "error": {
        "code": "barcode_scan_timeout",
        "message": "扫码超时",
        "retryable": false
      }
    }
  }
}
```

## 设计建议

- 等待型插件的 `waitSignal.payload` 至少包含 `requestId` 和业务关联键，如 `taskId`、`orderNo`、`waveNo`。
- 回调入口应先用 `requestId`、业务键或插件任务 id 定位运行记录，再调用 `/runs/{runId}/resume`。
- `waitSignal.type` 应是稳定事件名，不要包含一次性 id；一次性 id 放在 `payload`。
- descriptor 的 `kind` 可以写 `wait` 来获得等待节点视觉语义，但 workflow 节点 `type` 必须保持 `plugin:*`。
- 对已发布插件，新增配置字段优于重命名字段；重命名会破坏旧 workflow 的 `config`。
- `configSchema.properties` 中的默认值要与插件服务端默认行为一致，避免前端预览与实际执行不一致。
