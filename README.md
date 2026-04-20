# SES Flow

SES Flow 是一个包含前端、后端和工作流运行内核的多应用仓库：

- `apps/ai-gateway`：Node + Express AI 网关，承接页面内 Claude Agent SDK 会话
- `apps/backend`：Rust + Axum Web 服务，对外提供 `/runner-api` 与 `/views`
- `apps/frontend`：Vue 3 + Vite 工作流编辑与预览界面
- `apps/runner`：Rust 工作流执行内核库，由 `backend` 直接依赖
- `packages/scheme`：前端共享 schema

## 常用命令

> 由于包含多个应用和库，建议使用 `just` 编排常用命令，避免频繁切换目录执行。
> 安装just：https://just.systems/man/zh/%E5%AE%89%E8%A3%85%E5%8C%85.html
> 安装Rust：https://rustwiki.org/zh-CN/cargo/getting-started/installation.html

```bash
just dev
just build
just test
just lint

cargo test --workspace
cargo build --workspace
just dev-backend
just dev-frontend
just dev-ai-gateway
```

根目录脚本通过 `just` 编排，请先在本机安装 `just`。

## 本地访问

- `pnpm dev` 启动后：
  - AI Gateway：`http://127.0.0.1:6307`
  - 前端开发服务：`http://127.0.0.1:5173/views/`
  - Backend：`http://127.0.0.1:6302`
  - Workflow API：`http://127.0.0.1:6302/runner-api`
  - AI 协作代理入口：`http://127.0.0.1:6302/api/ai/*`
- `pnpm start` 或容器部署后：
  - AI Gateway：`http://127.0.0.1:6307`
  - 前端静态页面：`http://127.0.0.1:6302/views/`
  - Workflow API：`http://127.0.0.1:6302/runner-api`
  - AI 协作代理入口：`http://127.0.0.1:6302/api/ai/*`

## AI 协作环境变量

- `AI_GATEWAY_HOST`：可选，默认 `127.0.0.1`。
- `AI_GATEWAY_PORT`：可选，默认 `6307`。
- `AI_GATEWAY_PROXY_TARGET`：可选。若设置，backend 会优先把 `/api/ai/*` 转发到这个完整地址。
- `CLAUDE_CODE_EXECUTABLE`：可选。指定本地 Claude Code 可执行文件路径。

AI 供应商信息不再从 `.env` 读取。页面内每次发起 AI 请求时，都必须由用户在前端配置 `baseUrl`、`authToken`、`model` 并随请求一起发送。

前端开发模式下，Vite 会把 `/runner-api/*` 和 `/api/ai/*` 都代理到 backend，保持和生产环境一致。

## 部署

- 本地容器编排：`docker compose up -d --build`
- 远端 SSH 部署：`scripts/deploy-apps-ssh.sh`
- 详细说明见 [docs/docker-deploy.md](./docs/docker-deploy.md)
