---
title: SES Flow 帮助中心
description: Workflow Builder 的系统说明、节点参考与 AI Agent 协作说明。
---

# SES Flow 帮助中心

这里整理了当前 SES Flow 的核心帮助说明，按主题拆分为独立页面，便于后续持续补充。

## 帮助目录

1. [系统功能说明](/#/help/system-features)
2. [节点说明](/#/help/node-reference)
3. [AI Agent 模式与 Skill 说明](/#/help/ai-agent-and-skills)

## 当前整理思路是否合理

这次按“功能入口、节点能力、AI 协作”拆分是合理的，原因有三点：

- 使用者通常先关心系统怎么用，再关心节点怎么配，最后才是 AI Agent 协作方式。
- 节点说明依赖 Runner 的真实执行语义，单独分页更方便后续和运行时一起更新。
- AI Agent 模式本身不是普通编辑模式的简单扩展，它涉及 `runner_base_url`、`session_id`、会话同步和 `ses-flow-skill`，独立成页更清晰。

## 建议继续补充的主题

下面这些内容目前还值得单独扩展成后续帮助页：

- 运行与调试：如何启动运行、查看 timeline、处理 `waiting` 状态、恢复执行。
- 变量与映射：`trigger / input / state / env` 的取值规则与常见模板写法。
- 发布与版本：草稿、发布、重新运行、JSON 导出各自适用场景。
- 常见问题：Runner 不可达、AI 会话创建失败、Webhook 与 `respond` 节点的当前限制。

## 快速入口

- [新建工作流](/#/workflow/new)
- [工作流列表](/#/workflow-list)
- [系统功能说明](/#/help/system-features)
- [节点说明](/#/help/node-reference)
- [AI Agent 模式与 Skill 说明](/#/help/ai-agent-and-skills)

---

继续阅读：[系统功能说明](/#/help/system-features)
