# 编辑会话 API

## 用途

这些端点为 SES Flow 的 AI 模式提供支撑。

- Claude Code 在 runner 中更新草稿。
- Runner 负责校验并存储临时草稿。
- Web 订阅会话并刷新预览。

## 调用前缀

首次进入 AI 会话时，除了 `session_id`，还必须同时提供 `runner_base_url`。

- `runner_base_url` 是以下所有 HTTP / WS 接口的请求前缀。
- 常见值可以是 `/runner-api`，也可以是完整地址，例如 `http://localhost:3000/runner-api`。
- 后续如果前缀未变，可以继续沿用同一个 `runner_base_url`。

## 创建

`POST {runner_base_url}/edit-sessions`

请求体：

```json
{
  "workspaceId": "ses-workflow-editor",
  "workflowId": "wf-optional",
  "editorDocument": {
    "schemaVersion": "1.0",
    "editor": {
      "pageMode": "ai"
    }
  },
  "workflow": {
    "meta": {
      "key": "sorting-main-flow",
      "name": "sorting-main-flow",
      "version": 3,
      "status": "draft"
    },
    "trigger": {
      "type": "manual"
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
}
```

响应字段：

- `sessionId`
- `workspaceId`
- `workflowId`
- `workflow`
- `editorDocument`
- `createdAt`
- `updatedAt`

## 更新

`PUT {runner_base_url}/edit-sessions/{session_id}`

请求体与创建接口一致。

说明：

- 发送完整的 `workflow`，不要只传局部补丁。
- `editorDocument` 是可选的，但为了获得准确的画布预览，建议一并发送。
- Runner 会在保存前校验工作流。

## 预览流

`WS {runner_base_url}/edit-sessions/{session_id}/ws`

消息结构：

```json
{
  "sessionId": "sess-123",
  "eventType": "snapshot",
  "session": {
    "sessionId": "sess-123",
    "workflowId": "wf-123",
    "workflow": {},
    "editorDocument": {},
    "createdAt": "2026-04-14T00:00:00Z",
    "updatedAt": "2026-04-14T00:00:00Z"
  }
}
```

当前 `eventType` 的取值包括：

- `snapshot`
- `created`
- `updated`

## AI 模式规则

- AI 模式下，Web 只用于预览。
- Claude Code 应承接编辑对话，并通过 runner 修改会话。
- 首次调用时，应同时拿到 `runner_base_url` 与 `session_id`，再拼接具体接口地址。
- 保持 `editor.editor.pageMode` 或等价的恢复状态与 AI 预览意图一致。
