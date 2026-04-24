# 降低现场排障成本 — 实现方案

> 目标：把"靠人翻日志排障"逐步变成"靠工具定位问题"

## 现状分析

| 能力 | 现状 |
|---|---|
| `runId` 日志追踪 | 已有，`engine.rs` 每条 tracing log 都带 `run_id` |
| Timeline 存储 | 已有，`timeline JSONB` 存于 `workflow_runs` / `workflow_snapshots` |
| `requestId`/`taskId` | 已存在于 trigger payload 和 node output，但**未被索引、未被联查** |
| 节点耗时 | `NodeExecutionRecord` **无 `started_at`/`ended_at` 字段** |
| 错误码/恢复建议 | **无结构化错误信息** |
| 联查 API | **不存在** |

---

## 阶段一：数据补全（基础，最高优先级）

### 1.1 `NodeExecutionRecord` 增加耗时与错误字段

**文件：** `apps/runner/src/core/runtime.rs`（NodeExecutionRecord 结构体）

新增字段：
```rust
pub started_at:   Option<DateTime<Utc>>,   // 节点开始时间
pub ended_at:     Option<DateTime<Utc>>,   // 节点结束时间
pub error_code:   Option<String>,          // 结构化错误码
pub error_detail: Option<String>,          // 原始错误信息
```

**改动点：**
- `engine.rs` 在 `dispatch_node` 前记录 `started_at`，完成/失败后记录 `ended_at` 和 `error_code`
- `error_code` 按错误类型枚举：`TIMEOUT` / `HTTP_ERROR` / `VALIDATION_FAILED` / `RESUME_MISMATCH` 等

### 1.2 `workflow_runs` 增加联查索引

**文件：** `apps/runner/src/store/postgres.rs`（migration）

```sql
-- 冗余列，提升联查性能
ALTER TABLE workflow_runs
  ADD COLUMN order_no   TEXT,
  ADD COLUMN wave_no    TEXT,
  ADD COLUMN request_id TEXT;

CREATE INDEX idx_runs_order_no   ON workflow_runs (order_no);
CREATE INDEX idx_runs_wave_no    ON workflow_runs (wave_no);
CREATE INDEX idx_runs_request_id ON workflow_runs (request_id);

-- taskId 存于 timeline 节点 output，GIN 索引支持 JSONB 内查询
CREATE INDEX idx_runs_timeline_gin ON workflow_runs USING GIN (timeline);
```

`run_service.rs` 的 `create_run` 写入时从 `trigger.headers`/`trigger.body` 提取 `orderNo`、`waveNo`、`requestId` 冗余写入新列。

---

## 阶段二：联查 API

### 2.1 新增 `/api/runs/search` 接口

**新文件：** `apps/backend/src/modules/run/run_search_service.rs`

```
GET /api/runs/search?orderNo=&waveNo=&runId=&requestId=&page=&pageSize=
```

返回结构：
```json
{
  "items": [
    {
      "runId": "run-xxx",
      "workflowKey": "wms-sorting",
      "status": "completed",
      "orderNo": "ORD-001",
      "waveNo": "WAVE-20240421",
      "requestId": "req-abc",
      "startedAt": "...",
      "endedAt": "...",
      "durationMs": 1234
    }
  ],
  "total": 42
}
```

### 2.2 Timeline 详情接口增强

现有 `GET /api/runs/:runId` 返回 timeline，补充以下字段：
```json
{
  "timeline": [
    {
      "nodeId": "node_1",
      "nodeType": "HttpRequest",
      "status": "completed",
      "startedAt": "...",
      "endedAt": "...",
      "durationMs": 230,
      "inputSummary": "POST /wms/pick-task {...}",
      "outputSummary": "taskId=task-xxx, status=created",
      "errorCode": null,
      "errorDetail": null,
      "recoveryHint": null
    }
  ]
}
```

`recoveryHint` 由 `error_code` 映射静态表得出：

| error_code | recoveryHint |
|---|---|
| `HTTP_ERROR` | 检查目标服务是否可用，查看 HTTP 状态码 |
| `TIMEOUT` | 检查网络延迟或目标服务响应时间 |
| `VALIDATION_FAILED` | 检查入参格式是否符合节点配置的 schema |
| `RESUME_MISMATCH` | 回调 requestId/taskId 不匹配，确认外部系统是否重复回调 |

---

## 阶段三：前端排障工作台

### 3.1 运行搜索页

**新页面：** `apps/frontend/src/pages/troubleshoot/`

功能：
- 搜索框支持 `runId` / `requestId` / 订单号 / 波次号
- 结果列表展示状态、耗时、触发时间
- 点击进入 Timeline 详情

### 3.2 Timeline 详情增强

**现有类型：** `apps/frontend/src/features/workflow/runner.ts`（WorkflowRunTimelineItem）

增强点：
- 每个节点卡片展示耗时条（progress bar 可视化）
- 展开节点显示 input/output 摘要
- 错误节点高亮 + `errorCode` + `recoveryHint`
- 节点状态颜色：`pending=灰` / `running=蓝` / `completed=绿` / `failed=红`

### 3.3 事件回放（仅展示，不重执行）

基于已有 `timeline JSONB`，按 `started_at` 排序，在前端按时序"播放"节点状态切换，帮助排障人员重现执行过程。

---

## 阶段四：故障模板与人工补录

### 4.1 常见故障排查模板

前端内置静态模板（JSON 配置），按 `workflowKey` + `errorCode` 组合展示排查步骤：

```json
{
  "wms-sorting": {
    "HTTP_ERROR": ["检查 WMS 接口 /pick-task 是否可用", "查看 HTTP 响应码", "检查网络策略"],
    "RESUME_MISMATCH": ["确认回调 URL 是否正确", "检查外部系统 taskId 是否与创建时一致"]
  }
}
```

### 4.2 人工补录接口

```
POST /api/runs/:runId/manual-patch
Body: { "nodeId": "node_1", "note": "人工确认已处理", "operator": "张工" }
```

补录记录追加到 `NodeExecutionRecord.logs`（现有字段），前端 Timeline 展示人工备注。

---

## 交付顺序

```
阶段一（1-2周）→ 阶段二 API（1周）→ 阶段三前端（2周）→ 阶段四（1周）
```

| 里程碑 | 交付物 | 价值 |
|---|---|---|
| 阶段一完成 | `NodeExecutionRecord` 增加耗时/错误码字段 + DB 索引 | 日志有耗时、有错误码，可在 DB 直接排查 |
| 阶段二完成 | 联查 API + Timeline 增强 API | 技术人员可用 API 按订单/波次/runId 定位 |
| 阶段三完成 | 排障工作台前端页面 | 现场实施人员可用 Web 工具自助排障 |
| 阶段四完成 | 故障模板 + 人工补录 | 支持协同处理、历史溯源、模板化排障 |
