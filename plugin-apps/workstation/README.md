# Workstation Plugin App

`plugin-apps/workstation` 是一个面向人工工作站 App 的 bridge 型 HTTP plugin，并保留兼容旧 WCS 协议的站点接口。

它做两件事：

- 对 `runner` 暴露标准动态插件协议：`GET /descriptors`、`GET /descriptor`、`GET /health`、`POST /execute`、`POST /cancel`、`POST /resume`
- 对工作站 App 暴露兼容旧 WCS 的接口：`/station/operation/login`、`/station/operation/connect`、`/station/operation/verifyNotify`、`/station/operation/scanBarcode`、`/station/operation/getTaskInfo`、`/station/operation/robotDeparture`

当前实现是首版可跑骨架，特点：

- 内存态任务与 pending event 管理
- `scan_task` / `pack_task` 两个 descriptor
- `POST /execute` 立即返回 `waiting`
- 工作站完成 `robotDeparture` / `noBarcodeForceDepart` 后，主动回调 runner `POST /runner-api/runs/{runId}/resume`
- `fail` 路径会把失败结果回灌给 runner

## 启动

```bash
cargo run -p workstation-plugin -- --host 127.0.0.1 --port 9102
```

可选环境变量：

- `RUNNER_BASE_URL=http://127.0.0.1:6302/runner-api`
- `WORKSTATION_HEARTBEAT_INTERVAL_SECS=15`

## 本地联调

1. 启动 backend。
2. 启动本插件。
3. 在 backend 注册插件 baseUrl，例如 `http://127.0.0.1:9102`。
4. workflow 使用 `plugin:scan_task` 或 `plugin:pack_task`。
5. 工作站 App 调 `POST /station/operation/login` 获取 token，再通过 `POST /station/operation/connect` 建立 SSE。

## 当前边界

- Pending queue 和 token 仍是内存实现，Bridge 重启后不会恢复。
- 还没有接真实 AGV/WCS/商品服务，`scanBarcode` 和 `getTaskInfo` 返回 mock 数据。
- runner 回调目前直接走 backend 的 `/runner-api/runs/{runId}/resume`，未接入 capability token。
