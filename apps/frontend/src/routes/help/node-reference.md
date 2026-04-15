---
title: 节点说明
description: 参考 Runner 当前实现整理的节点能力说明。
---

# 节点说明

本页主要参考 `apps/runner/README.md` 当前的节点支持说明，同时结合前端编辑器里已经暴露的节点能力进行整理。

## 如何理解“节点说明”

当前有两层能力需要区分：

1. Runner 运行时已经支持哪些节点。
2. 前端编辑器当前已经把哪些节点开放成可拖拽、可配置的节点。

这两层能力并不完全一致，所以帮助页里会分别说明。

## Runner 当前支持的节点

Runner README 当前列出的节点包括：

- `start`
- `end`
- `webhook_trigger`
- `respond`
- `fetch`
- `set_state`
- `switch`
- `code`
- `shell`
- `sub_workflow`
- `wait`
- `task`

## 前端编辑器当前可直接使用的节点

当前编辑器调色板里可直接添加的节点包括：

- Start
- End
- Webhook Trigger
- Respond
- Switch
- Sub-Workflow
- Fetch
- Shell
- Code
- Wait
- Task

## 节点分组说明

### 1. 流程控制

| 节点 | 作用 | 当前说明 |
| --- | --- | --- |
| Start | 流程入口 | 默认输出 `trigger.body`，如果没有 `body` 则输出整个 `trigger` |
| End | 流程结束 | 标记流程结束，通常用于正常完成 |
| Switch | 多分支路由 | 根据表达式结果选择不同分支，可设置默认分支 |
| Sub-Workflow | 调用子工作流 | 将当前输入传递给下游子流程 |

### 2. 触发与响应

| 节点 | 作用 | 当前说明 |
| --- | --- | --- |
| Webhook Trigger | 从触发上下文中选择载荷 | 当前更像“流程内触发数据选择节点”，不是自动创建真实 HTTP 路由 |
| Respond | 生成响应信号 | 适合与 webhook 场景配合使用，输出响应载荷 |

### 3. 数据与执行

| 节点 | 作用 | 当前说明 |
| --- | --- | --- |
| Fetch | 发起 HTTP 请求 | 支持 `GET` / `POST`，返回结构中可读取 `data` 和 `response.status` |
| Code | 执行内联代码 | 当前以 JavaScript / TypeScript 为主，可读 `trigger / input / state / env / params` |
| Shell | 调用本机命令 | `inputMapping` 会被序列化为输入参数，适合本地脚本处理 |
| Task | 发出任务并等待完成事件 | 默认以 `task.completed` 作为完成事件 |

### 4. 等待与异步

| 节点 | 作用 | 当前说明 |
| --- | --- | --- |
| Wait | 等待外部事件恢复执行 | 常用于设备回调、人工确认、任务完成等场景 |

## 重点节点的使用建议

### Start

- 用于定义流程统一入口。
- 后续节点一般直接从 `input` 里拿到 Start 的输出。

### Webhook Trigger

- 适用于流程由外部请求触发的场景。
- 当前配置里的 `path` 和 `responseMode` 主要仍是流程定义层面的元数据。
- 现阶段不要把它理解成“配置完就自动生成独立 webhook 路由”。

### Fetch

- 适合拉取外部接口数据。
- 通常下个节点会从 `{{input.data}}` 或 `{{input.response.status}}` 读取结果。

### Switch

- 适合按业务类型、状态、结果标签进行多路分流。
- 如果表达式结果无法匹配到明确分支，通常需要依赖默认分支兜底。

### Code

- 适合做中间转换、字段整形、结果拼装。
- 如果处理逻辑已经明显超出简单转换，建议谨慎评估是否应切到独立服务或 Shell。

### Shell

- 适合快速接本地脚本、系统命令或现有 CLI 工具。
- 需要注意本机运行环境、解释器、工作目录和超时。

### Wait

- 适合“发起动作后等待外部事件”的异步流程。
- 如果流程卡在 `waiting`，通常意味着还没有收到匹配的恢复事件。

### Task

- 适合把流程推进到外部任务系统后再回来继续执行。
- 当前帮助说明里建议把它理解成“面向任务完成事件的异步节点”。

## 变量与数据流的通用规则

Runner README 中有几个很重要的通用约定：

- 节点可通过模板访问 `trigger / input / state / env`。
- 如果没有配置 `inputMapping`，默认会把上一个节点的输出作为当前输入。
- `switch` 这类分支节点会通过 `label`、`condition` 或默认分支来匹配流向。
- `code`、`sub_workflow` 等节点可以产出 `statePatch`，供后续节点继续使用。

## 当前值得特别说明的差异

下面这些差异建议在后续产品迭代中持续关注：

- `set_state` 已经在 Runner 侧支持，但当前前端调色板还没有直接暴露。
- `if / else` 数据结构在前端里有准备，但当前调色板未开放为正式节点。
- 某些字段在前端里已经有配置入口，但还没有和运行时能力完全一一映射。

## 建议后续补充

节点说明页后续可以继续补充这些内容：

- 每个节点的最小 JSON 示例。
- 每个节点的输入与输出样例。
- 常见错误案例，例如 `wait` 恢复事件不匹配、`fetch` 请求失败、`shell` 超时。

---

上一页：[系统功能说明](/views/#/help/system-features)

继续阅读：[AI Agent 模式与 Skill 说明](/views/#/help/ai-agent-and-skills)
