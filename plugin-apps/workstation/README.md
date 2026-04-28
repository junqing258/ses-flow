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
- `DATABASE_URL=postgresql://...`
- `WORKSTATION_HEARTBEAT_INTERVAL_SECS=5`

## 本地联调

1. 启动 backend。
2. 启动本插件。
3. 在 backend 注册插件 baseUrl，例如 `http://127.0.0.1:9102`。
4. workflow 使用 `plugin:scan_task` 或 `plugin:pack_task`。
5. 工作站 App 调 `POST /station/operation/login` 获取 token，再通过 `POST /station/operation/connect` 建立 SSE。
6. 手动模拟小车到达可调用：

```bash
curl -X POST http://127.0.0.1:9102/station/operation/simulate/agvArrived \
  -H 'Content-Type: application/json' \
  -d '{"stationId":"juFomZRB","agvId":"AGV-001","requestId":1001}'
```

该接口会向对应 `stationId` 的 SSE 连接推送兼容旧客户端的 `AGV_ARRIVED` 事件；如果配置了 `DATABASE_URL` 和 `RUNNER_BASE_URL`，还会直接查询 `workflow_runs` 中 `lastSignal.type=agv.arrived`、`lastSignal.payload.stationId` 且 `requestId` 相同的等待中 runId，并调用 `/runs/{runId}/resume` 推进“等待小车到达”。`scanBarcode` 成功时同样会按 `lastSignal.type=station.operation.scanBarcode`、`stationId` 和 `requestId` 查询等待 run，并携带 `requestId/barcode/itemId/sku` 恢复流程。

## 当前边界

- Pending queue 和 token 仍是内存实现，Bridge 重启后不会恢复。
- 还没有接真实 AGV/WCS/商品服务，`scanBarcode` 和 `getTaskInfo` 返回 mock 数据。
- runner 回调目前直接走 backend 的 `/runner-api/runs/{runId}/resume`，未接入 capability token。
