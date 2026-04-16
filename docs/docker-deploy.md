# Docker 部署说明

当前 Docker 部署只启动一个 `backend` 服务：

- `backend`：Rust + Axum Web 服务
- workflow 执行内核由 `backend` 内部依赖的 `runner` library 提供
- 前端静态资源会在镜像构建时打包进 `backend`，并通过 `/views/` 提供访问

PostgreSQL 不在本仓库 Docker 编排内启动，`backend` 通过 `DATABASE_URL` 连接已有数据库。

## 1. 准备环境变量

```bash
cp .env.example .env
```

按你的环境修改 `DATABASE_URL`，例如：

```dotenv
DATABASE_URL=postgresql://runner:runner@host.docker.internal:5432/flow-runner
BACKEND_PORT=6302
```

## 2. 启动服务

```bash
docker compose up -d --build
```

启动后默认访问地址：

- 前端：`http://localhost:6302/views/`
- Workflow API：`http://localhost:6302/runner-api`

## 3. 停止服务

```bash
docker compose down
```

## 4. 远端 SSH 部署

仓库提供了一个本地构建并远程部署的脚本：

```bash
scripts/deploy-runner-ssh.sh
```

默认行为：

- 本地构建 `apps/backend/Dockerfile`
- 默认部署到 `root@192.168.110.45`
- 自动探测远端 CPU 架构并构建对应镜像
- 通过 SSH 将镜像传到远端并执行 `docker load`
- 上传 [docker-compose.remote.yml](/Users/junqing/git-hy/ses-flow/scripts/docker-compose.remote.yml) 和本地 `.env`
- 在远端执行 `docker compose up -d --force-recreate`

## 5. 实现说明

- `backend` 根路径 `/` 会跳转到 `/views/`
- `backend` 直接提供 `/views/*` 静态资源，兼容前端 `base: "/views/"`
- `backend` 运行镜像保留 Node.js 运行时，以兼容 `runner` 的 `code` 节点执行
- `runner` 现在是纯 Rust library crate，不再作为独立 HTTP 服务部署
