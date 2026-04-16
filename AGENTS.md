# Repository Guidelines

## 项目结构与模块组织
本仓库是一个基于 `pnpm` + `just` 的工作区：

- `apps/backend`：Rust + Axum 后端服务，负责提供 `/runner-api` 与 `/views`
- `apps/frontend`：Vue 3 + Vite 前端，业务代码在 `src/`，测试位于 `tests/**/*.test.ts`
- `apps/runner`：Rust 工作流运行内核库，核心逻辑在 `src/`，示例流程在 `examples/`
- `packages/scheme`：前端共享 schema 包

不要提交生成产物，如 `dist/`、`coverage/`、`target/` 或 `node_modules/`。

## 构建、测试与开发命令
以下命令在仓库根目录执行：

- `pnpm install`：安装整个工作区依赖
- `pnpm dev`：通过 `just` 并行启动 frontend + backend 开发任务
- `pnpm build`：通过 `just` 执行工作区构建
- `pnpm test`：通过 `just` 执行工作区测试
- `just dev-backend`：单独启动 backend 热更新开发服务
- `just dev-frontend`：单独启动 frontend 开发服务
- `cargo test --workspace`：运行 Rust workspace 测试
- `cargo build --workspace`：构建 Rust workspace

常用子项目命令：

- `cargo run -p backend -- --host 127.0.0.1 --port 6302`：启动 backend
- `cargo test -p backend`：运行 backend 测试
- `cargo test -p runner --lib`：运行 runner 库测试
- `pnpm --filter frontend dev`：启动 Vite 前端

## 代码风格与命名约定
遵循各应用现有风格，不要强行统一：

- 前端 TypeScript/Vue 使用双引号和分号
- Rust 使用 `rustfmt` 默认格式

文件命名保持现有模式。Rust 模块按语义目录组织；Vue 单文件组件使用 PascalCase，例如 `AuthDialog.vue`。

## 测试规范
backend 与 runner 使用 Rust 测试；frontend 使用 Vitest。

提交 PR 前，先运行与改动最相关的检查；如果改动跨多个应用，再补充工作区级别验证，例如：`pnpm test`、`pnpm lint`、`cargo test --workspace`、`cargo build --workspace`。

## 提交与合并请求规范
最近提交记录采用带 scope 的 Conventional Commits，例如 `feat(runner): ...`、`feat(backend): ...`、`test(runner): ...`，新提交请保持一致。

PR 应包含简要说明、受影响路径、已执行命令，以及配置或部署说明。涉及 UI 变更时附截图；涉及 API 契约、Docker 入口或工作流示例更新时请明确标注。

## 安全与配置提示
本地后端配置直接使用仓库根 `.env`。`*.env` 文件不得提交；根脚本依赖系统安装 `just`；涉及后端入口或数据库连接调整时，至少验证 `cargo test --workspace`。
