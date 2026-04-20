# Docker 部署说明

当前 Docker 部署会启动两个服务：

- `backend`：Rust + Axum Web 服务，对外提供 `/runner-api`、`/views/` 与 `/api/ai/*` 代理入口
- `ai-gateway`：Node + Express AI 网关，承接 Claude Agent SDK 会话
- workflow 执行内核由 `backend` 内部依赖的 `runner` library 提供
- 前端静态资源会在 `backend` 镜像构建时打包进去，并通过 `/views/` 提供访问

PostgreSQL 不在本仓库 Docker 编排内启动，`backend` 通过 `DATABASE_URL` 连接已有数据库。

## 1. 准备环境变量

```bash
cp .env.example .env
```

按你的环境修改 `DATABASE_URL`，例如：

```dotenv
DATABASE_URL=postgresql://runner:runner@host.docker.internal:5432/flow-runner
BACKEND_PORT=6302
AI_GATEWAY_PORT=6307
```

## 2. 启动服务

```bash
docker compose up -d --build
```

启动后默认访问地址：

- 前端：`http://localhost:6302/views/`
- Workflow API：`http://localhost:6302/runner-api`
- AI 协作代理入口：`http://localhost:6302/api/ai/*`
- AI Gateway 直接健康检查：`http://localhost:6307/health`

## 3. 停止服务

```bash
docker compose down
```

## 4. 远端 SSH 部署

仓库提供了一个本地构建并远程部署的脚本：

```bash
scripts/deploy-apps-ssh.sh
```

默认行为：

- 本地构建 `apps/backend/Dockerfile` 与 `apps/ai-gateway/Dockerfile`
- 默认部署到 `root@192.168.110.45`
- 自动探测远端 CPU 架构并构建对应镜像
- 通过 SSH 将两份镜像传到远端并执行 `docker load`
- 上传 [docker-compose.remote.yml](../scripts/docker-compose.remote.yml) 和本地 `.env`
- 在远端执行 `docker compose up -d --force-recreate`

## 5. 实现说明

- `backend` 根路径 `/` 会跳转到 `/views/`
- `backend` 直接提供 `/views/*` 静态资源，兼容前端 `base: "/views/"`
- `backend` 会把 `/api/ai/*` 转发到容器内 `ai-gateway`
- `ai-gateway` 在 Docker 中默认使用 `http://backend:6302/runner-api` 访问 runner API，避免容器内 `localhost` 回环到自身
- `backend` 运行镜像保留 Node.js 运行时，以兼容 `runner` 的 `code` 节点执行
- `runner` 现在是纯 Rust library crate，不再作为独立 HTTP 服务部署
