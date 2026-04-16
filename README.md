# SES Flow

SES Flow 是一个包含前端、后端和工作流运行内核的多应用仓库：

- `apps/backend`：Rust + Axum Web 服务，对外提供 `/runner-api` 与 `/views`
- `apps/frontend`：Vue 3 + Vite 工作流编辑与预览界面
- `apps/runner`：Rust 工作流执行内核库，由 `backend` 直接依赖
- `packages/scheme`：前端共享 schema

## 常用命令

```bash
pnpm install
pnpm dev
pnpm build
pnpm test
pnpm lint

cargo test --workspace
cargo build --workspace
just dev-backend
just dev-frontend
```

根目录脚本通过 `just` 编排，请先在本机安装 `just`。

## 本地访问

- `pnpm dev` 启动后：
  - 前端开发服务：`http://127.0.0.1:5173/views/`
  - Workflow API：`http://127.0.0.1:6302/runner-api`
- `pnpm start` 或容器部署后：
  - 前端静态页面：`http://127.0.0.1:6302/views/`
  - Workflow API：`http://127.0.0.1:6302/runner-api`

## 部署

- 本地容器编排：`docker compose up -d --build`
- 远端 SSH 部署：`scripts/deploy-runner-ssh.sh`
- 详细说明见 [docs/docker-deploy.md](/Users/junqing/git-hy/ses-flow/docs/docker-deploy.md)
