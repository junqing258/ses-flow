# 人工工作台 Bridge 实施方案（SSE 版）

> 本文是 [dynamic-node-registry.md](./dynamic-node-registry.md) 路径 B 的子方案。
> 面向场景：人工工作台（PDA、手机 App、Web 工作台）没有 HTTP Server，runner 无法直接调用；同时业务要求 runner 能把人工任务派给特定工人，工人完成/挂起/取消后回传结果。

---

## 背景与目标

### 约束

1. **人工 App 不能作为 HTTP 被调用方**（NAT / 移动网络 / 生命周期不稳定）。
2. **Runner 不想新增反向能力**。动态节点方案的核心收益是"runner 只作为 HTTP 调用方调用标准 plugin 协议"，这个抽象必须保留。
3. **多工人并发**。一个仓库 N 个工人在线，每个工人当下可能有 0 或 1 个活跃人工节点。
4. **弱网容忍**。工人切换 WiFi / 进出信号盲区是常态，断连重连不能丢任务。

### 目标

- **对 runner 透明**：Bridge 对外实现完整 HTTP 插件协议（`/descriptor`、`/health`、`/execute`、`/cancel`、`/resume`），runner 当成普通 HTTP 插件使用。
- **对 App 友好**：App 只需发 outbound HTTP + 订阅一条 SSE 流。
- **无会话丢失**：App 离线/重连不丢任务，幂等可重投。
- **一套 Bridge 服务所有人工节点类型**：新增"拣货确认""异常复核""称重录入"等都不需要改 Bridge 核心。

---

## 整体架构

```
┌────────┐  HTTP plugin protocol   ┌────────────┐     SSE (push)    ┌──────────┐
│ runner │ ──────────────────────▶ │            │ ────────────────▶ │          │
│        │ ◀────────────────────── │   Bridge   │                   │   App    │
└────────┘   /execute 同步响应     │            │ ◀──── HTTP POST ─ │ (工作台) │
    ▲                              │  Session   │   ack / result    └──────────┘
    │                              │  Manager   │
    │       Host API (反向)        │  Pending   │
    └─────────────────────────────▶│   Queue    │
                                   └────────────┘
                                         │
                                         ▼
                                   ┌────────────┐
                                   │  Postgres  │
                                   │  (sessions,│
                                   │   pending) │
                                   └────────────┘
```

三段通信：

1. **Runner ↔ Bridge**：标准 HTTP plugin 协议，完全复用 [dynamic-node-registry.md](./dynamic-node-registry.md) 已定义的 request/response 结构。
2. **Bridge → App**：SSE 单向推送（任务派发、取消、恢复事件）。
3. **App → Bridge**：普通 HTTP POST（登录、心跳、ack、result、Host API 代理调用）。

> SSE 为什么够用：本场景里需要"服务端推送"的只有 runner → App 方向；App → Bridge 是请求-响应式的，走普通 HTTP 即可。WebSocket 的全双工能力在这里是过度设计。

---

## 组件职责

### Bridge 进程内

| 组件 | 职责 |
|---|---|
| **Plugin API Handler** | 对 runner 暴露标准 plugin 协议；把 runner 请求翻译成 Session 事件 |
| **App API Handler** | 登录、SSE 长连、ack/result 回传、Host API 代理 |
| **Session Manager** | 维护 `workerId ↔ SSE channel` 映射；管理在线状态 |
| **Pending Queue** | 持久化未 ack 的事件；App 重连时回放 |
| **Task Store** | 持久化 `executionId`（= 一次节点执行）状态机 |
| **Host API Proxy** | 把 App 发来的 state/log 调用鉴权后转发给 runner Host API |

### 外部依赖

- **Postgres**（或等价 KV）：存 pending events、task 状态、session 元信息。
- **Runner Host API**：已有，Bridge 作为透传代理。

---

## 数据模型

### ExecutionTask（Bridge 侧"一次节点执行"的主记录）

```jsonc
{
  "executionId": "exec-uuid",           // Bridge 内部主键
  "runId": "run-abc",                    // 来自 runner
  "requestId": "req-001",
  "nodeId": "node-123",
  "traceId": "a1b2c3d4e5f6",

  "pluginType": "plugin:manual_pick",   // 具体人工节点类型
  "targetWorkerId": "worker-42",        // 从 config.workerId 或输入解析
  "payload": { /* runner 传来的 config + input */ },

  "state": "pending"
      | "dispatched"                    // 已推送给 App
      | "in_progress"                   // App 已 ack 开始处理
      | "waiting_external"              // waiting 状态，等外部事件
      | "succeeded" | "failed" | "canceled",

  "hostToken": "eyJ...",                 // runner 下发的 capability token
  "hostApiBaseUrl": "http://runner:7788",

  "createdAt": "...",
  "updatedAt": "...",
  "expiresAt": "..."                     // timeoutMs + 宽限期
}
```

### PendingEvent（待 App 消费的 SSE 事件）

```jsonc
{
  "eventId": "evt-uuid",                // 单调递增；既是 SSE id 又是幂等键
  "workerId": "worker-42",
  "executionId": "exec-uuid",
  "type": "task.dispatch" | "task.cancel" | "task.resume",
  "payload": { ... },
  "createdAt": "...",
  "ackedAt": null                        // App ack 后写入；满足条件后清理
}
```

---

## 接口清单

### 一、Runner → Bridge（标准 plugin 协议，零扩展）

按 [dynamic-node-registry.md](./dynamic-node-registry.md) 已有约定实现。Bridge 行为：

| 接口 | Bridge 处理逻辑 |
|---|---|
| `GET /descriptor` | 返回 `supportsCancel: true, supportsResume: true, timeoutMs: 0`（0 = 不超时，由业务或工作流显式取消）|
| `GET /health` | 检查 DB 可用、在线 App 数；返回 200/503 |
| `POST /execute` | 创建 ExecutionTask → 解析 `targetWorkerId` → 入 Pending Queue → 推 SSE → **立即返回 `status: "waiting"`**，`waitSignal.type = "human_task_done"` |
| `POST /cancel` | 查 ExecutionTask → 入 Pending Queue `task.cancel` → 推 SSE → 返回 200 |
| `POST /resume` | runner 不会直接调（见下）|

**关键点**：人工节点永远走 `waiting` 语义。Bridge 收到 `/execute` 后不等工人完成，立刻返回 `waiting`，runner 挂起 workflow；工人完成后 Bridge **主动调 runner 的 resume 入口**把工作流推进。

> 按 [dynamic-node-registry.md](./dynamic-node-registry.md) 约定，resume 是"插件主动通知 runner"，Bridge 在这里就是这个"插件"。

### 二、App → Bridge（App 是 HTTP 客户端）

所有请求带 `Authorization: Bearer <appToken>`，`appToken` 绑定 `workerId`。

| 接口 | 说明 |
|---|---|
| `POST /app/v1/login` | 设备登录；body: `{ workerId, deviceId, appVersion }`；返回 `appToken` + `ssePath` |
| `GET  /app/v1/stream?since=<eventId>` | **SSE 长连**；`since` 为上次收到的最后 eventId，用于断线续传 |
| `POST /app/v1/ack` | 确认收到事件；body: `{ eventId }`；Bridge 从 Pending 删除 |
| `POST /app/v1/tasks/:executionId/progress` | 上报进度；透传到 runner Host API `/host/v1/progress` |
| `POST /app/v1/tasks/:executionId/logs` | 批量推结构化日志；透传到 `/host/v1/logs` |
| `POST /app/v1/tasks/:executionId/state` | State patch；透传到 `PATCH /host/v1/state` |
| `POST /app/v1/tasks/:executionId/complete` | 人工任务完成；body: `{ output, statePatch? }`；Bridge 触发 resume-to-runner |
| `POST /app/v1/tasks/:executionId/fail` | 人工任务失败；body: `{ error: { code, message } }` |
| `POST /app/v1/heartbeat` | 可选；SSE 连接本身也可作心跳源 |

### 三、Bridge → App（SSE 事件流）

SSE 格式统一：

```
id: <eventId>
event: <type>
data: <json payload>

```

事件类型：

| event | 含义 | data 关键字段 |
|---|---|---|
| `task.dispatch` | 新任务派发 | `executionId, pluginType, payload, expiresAt` |
| `task.cancel` | runner 要求取消正在进行的任务 | `executionId, reason` |
| `task.resume` | 外部系统通过 runner resume 回传的信号（罕见，多数人工节点是 App 自己完成）| `executionId, signal` |
| `sync.snapshot` | 重连后一次性下发当前 workerId 的活跃任务快照 | `tasks: [ExecutionTask 摘要]` |
| `ping` | 每 15s 一次；保活 + 让 App 检测断连 | `ts` |

### 四、Bridge → Runner（resume 回灌）

Bridge 完成人工任务后，调 runner 的标准 resume 入口：

```
POST {runner}/runs/{runId}/nodes/{nodeId}/resume
Authorization: Bearer <hostToken>
X-Trace-Id: <traceId>

{
  "requestId": "req-001",
  "signal": {
    "type": "human_task_done",
    "payload": { "output": {...}, "statePatch": {...} }
  }
}
```

> 该端点是 runner 侧已有的人工复核等待回调入口（见 commit `132d478`），Bridge 直接复用。

---

## 核心流程

### 流程 1：正常派发与完成

```
runner            Bridge                      App
  │                 │                           │
  │─ POST /execute ▶│                           │
  │                 │─ 建 ExecutionTask         │
  │                 │─ 入 Pending Queue         │
  │                 │─ push SSE task.dispatch ─▶│
  │◀ waiting ───────│                           │
  │ (workflow 挂起) │                           │
  │                 │◀── POST /ack ─────────────│
  │                 │◀── POST /complete ────────│
  │                 │─ 透传 statePatch 到       │
  │                 │   Host API (可选)         │
  │                 │                           │
  │◀─ resume callback from Bridge ──────────────│
  │   (workflow 恢复推进)
```

### 流程 2：App 断连重连（弱网）

```
App ── SSE 断开 ──X
     (30s 后 App 发现心跳丢失)
App ── GET /app/v1/stream?since=<lastEventId> ─▶ Bridge
                                                   │
                Bridge 从 Pending Queue 读取         │
                workerId 下所有 eventId > since 的   │
                未 ack 事件，按序重放                 │
                                                   │
App ◀── replay: task.dispatch / task.cancel ──────│
App ◀── sync.snapshot（当前活跃任务全量）──────────│
```

**幂等保证**：App 侧按 `executionId + eventId` 去重；任务状态机在 App 本地也维护，重复 dispatch 只更新视图不重复处理。

### 流程 3：取消

```
runner ──▶ Bridge /cancel
            │
            ├─ 更新 ExecutionTask.state = canceling
            ├─ 入 Pending Queue task.cancel
            └─ push SSE ──▶ App（当场或重连时收到）
                             │
                             └─ App UI 提示 + 清理本地
                                 │
App ──▶ POST /tasks/:id/fail { error.code: "canceled" }
            │
Bridge ─▶ runner resume（带 status:failed 信号）
```

> 语义选择：首期统一按"App 必须回 fail/complete"闭环，runner 侧统一通过 resume 收尾，避免 Bridge 要实现多个反向入口。

### 流程 4：工人离线时派发

```
runner ──▶ Bridge /execute (targetWorkerId = worker-42)
            │
            ├─ worker-42 当前无 SSE 连接
            ├─ 入 Pending Queue（持久化）
            └─ 返回 waiting（runner 挂起）
                                              worker-42 上线
                                                   │
                                  GET /stream?since=0
                                                   │
            Bridge 回放 Pending Queue ─────────────▶│
```

---

## 鉴权

### Runner ↔ Bridge

走动态节点方案已有的 **capability token**。Bridge 在 ExecutionTask 中保存 `hostToken`，调 Host API 和 resume 时带上。

### App ↔ Bridge

- 登录换 appToken（短期，如 12h）。
- appToken 在 Bridge 侧绑定 `workerId`；所有 `/app/v1/tasks/:executionId/*` 请求都校验 `task.targetWorkerId == token.workerId`，防止跨人篡改。
- SSE 连接通过 `Authorization` header 或 query 参数 `?token=...`（视网关是否透传 header）。

### 不信任 App 侧写入

`POST /tasks/:executionId/state` 等 Host API 代理调用，Bridge 必须：

1. 校验 token 归属；
2. 按 `hostToken.scopes` 二次过滤；
3. 写入动作打 audit log。

---

## 持久化与一致性

### 必须持久化

- **ExecutionTask**：保证 Bridge 重启后 runner 的 `/cancel` 还能找到对应任务。
- **PendingEvent**：保证 App 在 Bridge 重启期间不丢任务。

### 不需要持久化

- **在线 SSE 连接表**：内存即可；Bridge 重启时 App 自动重连。
- **进度/日志**：已通过 Host API 透传到 runner，Bridge 不做副本。

### 幂等键

| 场景 | 幂等键 |
|---|---|
| runner 重试 `/execute` | `(runId, nodeId, requestId)` |
| App 重复 `/complete` | `executionId` + 客户端 `requestId` |
| SSE 事件重放 | `eventId` |

---

## 部署与扩展

### 单实例（首期）

- Bridge 单进程，Postgres 做持久化。
- SSE 连接数上限 ≈ 5000（tokio / libuv 都能轻松支撑）。
- 一个仓库通常 ≤ 200 工人，单实例足够。

### 多实例（中后期）

- SSE 连接粘性路由（workerId → 某个 Bridge 实例），用 Redis pub/sub 跨实例转发事件。
- Pending Queue 天然共享（Postgres）。
- Runner 端看到的 baseUrl 是 Bridge 前置的 LB。

### 横切关注点

- **日志**：Bridge 每条日志带 `runId + requestId + executionId + workerId`，与 runner 日志共享 `traceId`。
- **指标**：在线工人数、Pending 堆积、SSE 连接数、per-event 投递延迟。
- **健康检查**：`/health` 检查 DB 连接、SSE 协程池。

---

## 与现有方案的兼容性

| 方面 | 是否改动 |
|---|---|
| runner plugin executor | ❌ 零改动，Bridge 就是一个 `transport=http` 的插件 |
| NodeDescriptor 协议 | ❌ 零改动，人工节点与其他 HTTP 插件用同一份 schema |
| Host API | ❌ 零改动，Bridge 作为代理透传 |
| runner resume 入口 | ✅ 复用（commit `132d478` 已具备）|
| 日志格式 | ✅ 复用，`traceId` 贯穿 runner → Bridge → App |

---

## 实施阶段

### P0（MVP，~1 人周）

- Bridge 进程骨架（技术栈建议与 runner 对齐，用 Rust + axum）。
- `POST /execute` → `waiting` + Pending Queue + SSE 推送。
- App 侧：登录 + SSE 订阅 + `/complete`。
- Bridge → runner resume 回灌。
- 单实例、Postgres 持久化。

### P1（生产可用，~2 人周）

- `/cancel` 完整链路。
- SSE 断线续传（`since` 参数 + eventId 连续性）。
- Host API 代理（state/logs/progress）。
- 鉴权与 audit log。
- Prometheus 指标。

### P2（规模化）

- 多实例 + Redis pub/sub。
- 动态路由策略（按仓库、按区域分片）。
- App 端 SDK 统一封装（PDA / 移动端 / Web 复用）。
- gRPC 版本（与 runner 动态节点方案的 grpc transport 演进对齐）。

---

## 关键决策记录

| 决策 | 选型 | 原因 |
|---|---|---|
| App 通信协议 | **SSE + HTTP POST**，不用 WebSocket | App 只需单向接收推送；SSE 天然走 HTTP/HTTPS，穿透 NAT 和企业代理更稳；WebSocket 的全双工是过度设计 |
| Bridge 响应语义 | `/execute` 立刻返回 `waiting` | 人工任务时长不可预期（秒到小时），不能占用 runner 同步连接 |
| resume 路径 | Bridge 主动调 runner resume | 与 [dynamic-node-registry.md](./dynamic-node-registry.md) 已有约定一致 |
| 持久化存储 | Postgres | 与平台主库共用连接池；Pending Queue 体量小，不需要 Kafka/Redis Streams |
| 一个 Bridge 多种人工节点 | ✅ | `pluginType` + `configSchema` 区分，Bridge 核心与具体业务节点解耦 |
