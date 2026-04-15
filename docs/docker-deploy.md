# Docker 部署说明

本项目当前 Docker 部署只启动一个 `runner` 服务：

- `runner`：Rust 工作流运行服务
- 前端静态资源会在镜像构建时打包进 `runner`，并由 `runner` 直接通过 `/views/` 提供访问

PostgreSQL 不在本仓库 Docker 编排内启动，`runner` 只通过 `DATABASE_URL` 连接你已经存在的数据库。

## 1. 准备环境变量

复制一份部署环境变量：

```bash
cp .env.example .env
```

按你的环境修改 `DATABASE_URL`。

示例：

```dotenv
DATABASE_URL=postgresql://runner:runner@host.docker.internal:5432/flow-runner
RUNNER_PORT=6302
```

说明：

- 如果 PostgreSQL 跑在宿主机上，默认示例里的 `host.docker.internal` 可以直接使用。
- 如果 PostgreSQL 跑在别的机器或已有 Docker 网络里，把 `host.docker.internal` 替换成对应主机名或 IP 即可。

## 2. 启动服务

```bash
docker compose up -d --build
```

启动后默认访问地址：

- 前端: `http://localhost:6302/views/`
- Runner API: `http://localhost:6302/runner-api`

## 3. 停止服务

```bash
docker compose down
```

## 4. 实现说明

- `runner` 根路径 `/` 会自动跳转到 `/views/`。
- `runner` 会直接提供 `/views/*` 静态资源访问，适配当前前端的 Vite `base: "/views/"` 配置。
- `runner` 运行镜像保留了 Node.js 运行时，以兼容 `code` 节点执行。
- 仓库里没有可一起部署的 `backend` 服务，因此当前这套 Docker 方案只覆盖前端静态页面和 `runner-api`。
