# AI Gateway

AI Gateway 是 SES Flow 的 AI 协作服务，基于 Claude Agent SDK 提供工作流编辑的 AI 辅助功能。

## 功能特性

- 支持多种 AI 供应商（Anthropic、SiliconFlow 等）
- 基于 Claude Agent SDK 的代码理解和编辑能力
- 与 SES Flow Runner 的安全集成
- 流式响应和工具调用追踪

## 配置

AI Gateway 自身只负责承接会话和调用 Claude Agent SDK。AI 供应商配置不再从 `.env` 读取，而是由前端用户在每次请求中传入：

```bash
# 服务监听配置
AI_GATEWAY_HOST=127.0.0.1        # 可选，默认 127.0.0.1
AI_GATEWAY_PORT=6307             # 可选，默认 6307

# 可选：Claude Code 可执行文件路径
CLAUDE_CODE_EXECUTABLE=/path/to/claude
```

前端用户需要在配置弹窗中填写以下字段，并在每次请求中随 `aiProvider` 一起发送：

- `baseUrl`
- `authToken`
- `model`

如果这 3 个字段不完整，AI Gateway 会拒绝请求。

## 开发

```bash
# 安装依赖
pnpm install

# 开发模式
pnpm dev

# 运行测试
pnpm test

# 构建
pnpm build

# 生产运行
pnpm start
```

## API 端点

- `GET /health` - 健康检查
- `GET /api/ai/threads/:editSessionId` - 获取会话快照
- `GET /api/ai/threads/:editSessionId/events` - SSE 事件流
- `POST /api/ai/threads/:editSessionId/messages` - 发送消息
- `POST /api/ai/threads/:editSessionId/cancel` - 取消当前回合

## 架构

```
┌─────────────┐
│  Frontend   │
└──────┬──────┘
       │ HTTP/SSE
┌──────▼──────┐
│   Backend   │
│  /api/ai/*  │
└──────┬──────┘
       │ Reverse Proxy
┌──────▼──────┐
│ AI Gateway  │
├─────────────┤
│   Config    │ ← 读取请求里的 aiProvider
│   Service   │
│   Claude    │ ← Claude Agent SDK
│   State     │
└──────┬──────┘
       │ HTTP
┌──────▼──────┐
│   Runner    │
│ Edit Session│
└─────────────┘
```

## 安全限制

AI Gateway 对 Claude Agent SDK 的工具使用进行了严格限制：

- 只允许读取仓库内容（Read、Glob、Grep、LS）
- 只允许通过 `ses-flow-runner` MCP 工具访问当前 Runner Edit Session API
- 禁止修改仓库文件、提交代码或运行写文件命令
- 所有工作流修改必须通过 Runner API 完成
- Runner 工具请求默认 10 秒超时，避免工具调用长时间挂起

## 开发联调

- 前端在开发模式下访问 `/api/ai/*` 时，会先代理到 backend，再由 backend 反向代理到 AI Gateway。
- 生产环境下，推荐同样通过 backend 暴露 `/api/ai/*`，前端保持同源访问。

## 过程日志

AI Gateway 会输出 JSON 格式过程日志到 stdout/stderr，便于按 `editSessionId` 排查问题。关键事件包括：

- `http.thread.messages.post` / `http.thread.cancel.post`
- `thread.turn.requested` / `thread.turn.completed` / `thread.turn.failed` / `thread.turn.aborted`
- `thread.tool.started` / `thread.tool.completed`
- `thread.assistant.started` / `thread.assistant.completed`
- `thread.preview.updated`
