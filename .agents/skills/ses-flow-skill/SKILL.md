---
name: ses-flow-skill
description: 当通过 AI 模式编辑 SES Flow 工作流、处理 runner 编辑会话、创建或更新用于工作流预览的 session_id 与 runner_base_url，或推送 workflow/editorDocument 草稿以便只读 AI 模式下的 Web 画布刷新时，请使用此技能。
---

# SES Flow 技能

## 概述

这个技能用于本仓库中的 AI 驱动工作流编辑。

当任务涉及 SES Flow 工作流草稿、`session_id`、`runner_base_url`、runner 编辑会话，或 Web 编辑器中 AI 模式的预览刷新时，请使用它。

## 核心规则

- `Claude Code` 是编辑核心。在这里完成工作流分析与修改。
- `runner`服务 是 AI 会话草稿的事实来源。通过 runner API 进行校验。
- AI 模式下的 `web` 仅用于预览。不要依赖 Web 侧的编辑控件。
- 首次进入 AI 会话时，必须提供 `runner_base_url` + `session_id`，并将其作为所有会话接口请求的前缀。
- 在 AI 会话期间，应更新临时编辑会话，而不是已发布的工作流记录。
- 优先同时发送 `workflow` 和 `editorDocument`，这样预览才能恢复完整画布状态。
- AI 预览使用 HTTP `GET` 拉取会话快照。

## 何时使用

当用户要求以下内容时，请使用此技能：

- 通过 AI 模式编辑工作流
- 创建或使用 `session_id`
- 提供或使用 `runner_base_url`
- 向 Web 编辑器推送工作流预览更新
- 通过 runner 编辑会话修改节点、连线、面板、映射或工作流元数据
- 解释或实现 SES Flow 的 AI 编辑契约

## 工作流程

1. 确认当前工作流来源。  
   从仓库代码、当前 runner 载荷或用户提供的 `session_id` 与 `runner_base_url` 读取当前工作流上下文。

2. 解析编辑会话。  
   如果已经存在 `session_id`，就直接使用它。  
   如果缺少 `runner_base_url`，先向用户索取或从当前产品上下文中确认。  
   如果用户需要新的 AI 会话，则通过 `POST {runner_base_url}/edit-sessions` 创建。

3. 构建草稿载荷。  
   `workflow` 必须是完整的 runner 工作流定义。  
   `editorDocument` 应尽量包含图节点、连线、面板、当前选中节点、活动标签页，以及 `pageMode: "ai"`。  
   如果该会话绑定到现有工作流，请保留 `workflowId`。

4. 将草稿推送到 runner。  
   使用 `PUT {runner_base_url}/edit-sessions/{session_id}/draft` 更新已有会话。  
   以 runner 的校验失败结果为准，修复载荷后再重试。

5. 读取预览快照。  
   使用 `GET {runner_base_url}/edit-sessions/{session_id}` 获取最新会话快照。  
   当你刚更新完草稿、需要确认 runner 已接收变更，或要为 Web 只读画布提供最新状态时，都应读取这个接口。

6. 保持 Web 只读。  
   当 AI 模式处于激活状态时，不要让用户在浏览器中手动编辑。浏览器只应展示来自 runner 的最新预览。

## API 契约

当你需要请求或响应结构时，请阅读 [references/edit-session-api.md](references/edit-session-api.md)。

简要版本如下：

- 首次提供：`runner_base_url` + `session_id`
- 创建会话：`POST {runner_base_url}/edit-sessions`
- 更新会话：`PUT {runner_base_url}/edit-sessions/{session_id}/draft`
- 读取预览：`GET {runner_base_url}/edit-sessions/{session_id}`

## 数据与节点参考

详细格式请放到 `references/` 中查阅：

- [references/edit-session-api.md](references/edit-session-api.md)
- [references/workflow-json-format.md](references/workflow-json-format.md)
- [references/node-reference.md](references/node-reference.md)

推荐分工：

- `edit-session-api.md`
  看接口路径、请求骨架、更新规则和快照读取方式。
- `workflow-json-format.md`
  看 `workflow`、`editorDocument`、`transitions`、映射表达式和 runner 校验规则。
- `node-reference.md`
  看节点类型、用途、关键字段、前端对齐情况和分支约定。

最常用的记忆点：

- `workflow` 必须始终发送完整定义，不要发局部 patch。
- AI 预览最好始终同时发送 `editorDocument`。
- `editor.pageMode` 在 AI 预览里保持为 `"ai"`。
- 尽量保留工作流 id、版本和节点 id。
- 如果修改了节点 id，要同步修复 `transitions`、`graph.nodes`、`graph.edges`、`graph.panels`。

## 默认建议

- AI 预览文档中的 `pageMode` 保持为 `"ai"`。
- 如果前端已展示 `runner_base_url`，优先使用该值，不要自行猜测其他接口前缀。
- 除非任务明确要求修改，否则保留现有的工作流 id、名称、版本和节点 id。
- 更新会话后，可通过 `GET /edit-sessions/{session_id}` 验证 runner 中的最新草稿，而不是依赖本地内存状态。

## 避免事项

- 如果任务只是更新 AI 草稿，不要直接发布工作流。
- 不要在缺少 `runner_base_url` 的情况下假定请求前缀。
- 不要把 Web 状态视为高于 runner 会话状态的事实来源。
- 不要从 `editorDocument` 中移除字段，除非这些字段确实已明确废弃。
