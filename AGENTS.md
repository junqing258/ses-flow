# Repository Guidelines

## 项目结构与模块组织
本仓库是一个基于 `pnpm` + Turborepo 的工作区：

- `apps/backend`：NestJS + Fastify API，源码在 `src/`，Prisma schema 与迁移文件在 `prisma/`，Jest 测试位于 `test/**/*.spec.ts`。
- `apps/frontend`：Vue 3 + Vite 前端，业务代码在 `src/`，通用 UI 组件在 `src/components/ui`，Vitest 测试位于 `tests/**/*.test.ts`。
- `apps/runner`：Rust 工作流运行器，运行时代码在 `src/`，示例流程在 `examples/`。
- `packages/scheme`：用于跨应用共享 schema 的包。

不要提交生成产物，如 `dist/`、`coverage/`、`target/` 或 `node_modules/`。

## 构建、测试与开发命令
以下命令在仓库根目录执行：

- `pnpm install`：安装整个工作区依赖。
- `pnpm dev`：启动 Turbo 管理的开发任务。
- `pnpm build`：构建所有应用。
- `pnpm test`：运行所有已配置测试。
- `pnpm lint`：执行各应用的 lint 任务。
- `pnpm format` / `pnpm format:check`：格式化代码或检查 Prettier 格式。

常用子项目命令：

- `pnpm --filter backend dev`：启动 Nest 后端。
- `pnpm --filter frontend dev`：启动 Vite 前端。
- `pnpm --filter backend prisma:migrate:dev -- --name <name>`：创建 Prisma 迁移。
- `pnpm --filter runner test` 或 `cargo test`：运行 Rust runner 测试。

## 代码风格与命名约定
遵循各应用现有风格，不要强行统一：

- 后端 TypeScript 使用单引号和分号。
- 前端 TypeScript/Vue 使用双引号和分号。
- Rust 使用 `rustfmt` 默认格式。

文件命名应沿用现有模式，如 `*.module.ts`、`*.service.ts`、`*.spec.ts`、`*.test.ts`。Vue 单文件组件使用 PascalCase，例如 `AuthDialog.vue`；已采用小写命名的路由和工具模块保持原有风格。

## 测试规范
后端使用 Jest，测试文件位于 `test/**/*.spec.ts`；前端使用 Vitest，测试文件位于 `tests/**/*.test.ts`；Rust 测试与模块同目录放置，命名为 `*_tests.rs`。

提交 PR 前，先运行与改动最相关的检查；如果改动跨多个应用，再补充工作区级别验证，例如：`pnpm test`、`pnpm --filter frontend lint`、`pnpm --filter backend build`、`cargo test`。

## 提交与合并请求规范
最近提交记录采用带 scope 的 Conventional Commits，例如 `feat(runner): ...`、`test(runner): ...`，新提交请保持一致。

PR 应包含简要说明、受影响路径、已执行命令，以及配置或迁移说明。涉及 UI 变更时附截图；涉及 Prisma schema 或工作流示例更新时请明确标注。

## 安全与配置提示
本地后端配置请以 `apps/backend/.env.example` 为模板。`*.env` 文件不得提交；合并前请使用 `pnpm --filter backend prisma:validate` 校验 Prisma 变更。
