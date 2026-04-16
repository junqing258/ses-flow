# 编辑会话 API

## 用途

这些端点用于承载 SES Flow 的 AI 编辑草稿：

- Agent 修改 `workflow` 和 `editorDocument`
- Runner 校验并保存临时草稿
- Web 只读画布读取会话快照并刷新预览

## 调用前缀

首次进入 AI 会话时，必须同时获得：

- `runner_base_url`
- `session_id`

说明：

- `runner_base_url` 是所有编辑会话接口的请求前缀
- 常见值可以是 `/runner-api`，也可以是完整地址，如 `http://localhost:3000/runner-api`
- 后续同一会话只要前缀未变，就应继续沿用同一个 `runner_base_url`

## 接口列表

- 创建会话：`POST {runner_base_url}/edit-sessions`
- 更新草稿：`PUT {runner_base_url}/edit-sessions/{session_id}/draft`
- 获取快照：`GET {runner_base_url}/edit-sessions/{session_id}`

## 创建会话

`POST {runner_base_url}/edit-sessions`

### 请求体骨架

```json
{
  "workspaceId": "ses-workflow-editor",
  "workflowId": "wf-optional",
  "editorDocument": {},
  "workflow": {}
}
```

字段说明：

- `workspaceId`
  创建会话时建议显式传递；更新草稿时通常可以省略
- `workflowId`
  绑定已有工作流时传递；新草稿可以省略
- `editorDocument`
  用于恢复前端只读画布，建议始终一并发送
- `workflow`
  必填，且必须是完整的 runner 工作流定义

详细结构请见：

- [workflow-json-format.md](workflow-json-format.md)
- [node-reference.md](node-reference.md)

### 响应字段

成功时返回：

- `sessionId`
- `workspaceId`
- `workflowId`
- `workflow`
- `editorDocument`
- `createdAt`
- `updatedAt`

## 更新草稿

`PUT {runner_base_url}/edit-sessions/{session_id}/draft`

### 请求体

与创建接口相同，仍然是完整 upsert 请求体。

### 关键规则

- `workflow` 必须传完整定义，不支持局部 patch
- `editorDocument` 技术上可选，但 AI 预览场景强烈建议同时更新
- Runner 会在保存前校验工作流
- 若变更了节点 id、分支信息或节点布局，必须同步更新 `editorDocument.graph` 与 `workflow.transitions`

## 获取快照

`GET {runner_base_url}/edit-sessions/{session_id}`

### 响应结构

```json
{
  "sessionId": "sess-123",
  "workspaceId": "ses-workflow-editor",
  "workflowId": "wf-123",
  "workflow": {},
  "editorDocument": {},
  "createdAt": "2026-04-14T00:00:00Z",
  "updatedAt": "2026-04-14T00:00:00Z"
}
```

说明：

- 这是 AI 模式预览的标准读取接口
- Agent 在 `PUT` 之后可以立即 `GET` 一次，确认 runner 中保存的草稿是否符合预期
- Web 可以轮询该接口，或结合事件流刷新只读画布

## 相关参考

- [workflow-json-format.md](workflow-json-format.md)
- [node-reference.md](node-reference.md)

## AI 模式规则

- AI 模式下，Web 只用于预览
- Agent 应通过 runner 修改编辑会话，而不是依赖浏览器侧手动操作
- 首次调用时，应同时拿到 `runner_base_url` 与 `session_id`
- 预览读取统一使用 `GET /edit-sessions/{session_id}`
- 更新草稿时优先同时发送 `workflow` 和 `editorDocument`
- 若刚完成更新，建议立即读取一次快照，确认 runner 已保存最新状态
