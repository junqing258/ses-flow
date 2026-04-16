# Repository Guidelines

## Project Structure & Module Organization
本仓库是一个基于 `pnpm` + `just` 的工作区，Rust runner 位于 `apps/runner`。

- `src/`：运行时、执行引擎、API、存储和服务模块，例如 `engine.rs`、`server.rs`、`runtime.rs`。
- `src/*_tests.rs`：与模块同级的 Rust 测试文件，例如 `engine_tests.rs`、`template_tests.rs`。
- `examples/`：本地验证用的示例工作流定义和 JS handler，例如 `examples/code-flow.json`。
- `Cargo.toml`：Rust 包元数据与依赖声明。
- `package.json`：通过工作区脚本启动和测试 runner 的便捷入口。

不要提交生成产物，包括 `target/`、`dist/`、`coverage/` 和 `node_modules/`。

## Build, Test, and Development Commands
除非需要工作区级联验证，否则以下命令均在 `apps/runner` 下执行。

- `cargo run -- --host 127.0.0.1 --port 6302`：本地启动 runner 服务。
- `cargo test`：运行 runner 的全部 Rust 测试。
- `cargo build --release`：构建优化后的发布版本。
- `pnpm --filter runner dev`：通过工作区脚本启动 runner。
- `pnpm --filter runner test`：通过工作区脚本运行 runner 测试。

只有在改动影响多个应用时，才使用 `pnpm test` 这类工作区级命令。

## Coding Style & Naming Conventions
遵循 Rust 默认格式化规则，使用 `rustfmt`，并保持现有模块组织方式，不要额外引入新的命名风格。

- 文件、模块、函数和测试辅助方法优先使用 snake_case。
- 对外暴露的类型和 trait 使用 PascalCase。
- 文件命名应沿用现有模式，如 `api.rs`、`store.rs`、`services_tests.rs`。
- 修改范围应聚焦当前任务，不要顺手重构无关模块。

## Testing Guidelines
Rust 测试与源码同放在 `src/` 下，通常使用 `*_tests.rs` 命名。凡是执行逻辑、API 行为、模板渲染或持久化流程变更，都应补充或更新测试。

- 提交前至少运行一次 `cargo test`。
- 断言应尽量聚焦 workflow 状态、resume 行为和 API 返回。
- 新增或修改 `examples/` 时，要确认示例仍与当前 runner 行为一致。

## Commit & Pull Request Guidelines
提交信息使用带 scope 的 Conventional Commits，例如 `feat(runner): add wait event validation` 或 `test(runner): cover sub_workflow resume`。

PR 需包含简要说明、受影响路径、已执行命令，以及 workflow 或 API 行为变化。若 HTTP 接口行为有变更，附上示例请求或截图。

## Security & Configuration Tips
不要提交密钥、凭证或 `.env` 文件。`examples/` 只能放非敏感演示数据。若改动影响外部执行、resume 事件或 handler 加载方式，请在 PR 中明确运行时前提。
