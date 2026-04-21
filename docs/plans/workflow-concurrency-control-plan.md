# 工作流并发与任务控制实施方案

## 背景

当前 runner 通过 `tokio::task::spawn_blocking` 无限制地并发执行工作流，仅受 Tokio 默认线程池（512线程）约束，无任何队列或背压控制。

**核心文件**：
- `apps/runner/src/app/app.rs` — `start_workflow` / `resume_workflow` / `RunRegistry`

---

## 目标

| 维度 | 目标 |
|------|------|
| Workflow 级 | 限制单 workflow_key 的并发运行数，超限排队或拒绝 |
| 全局级 | 限制 runner 进程内同时运行的 workflow 总数 |
| 可配置 | 通过配置文件按 workflow_key 分别配置上限 |

---

## 方案设计

### 1. 并发策略枚举

```rust
pub enum OverflowPolicy {
    Queue,   // 超限排队，等待槽位释放
    Reject,  // 超限直接返回 429 错误
}
```

---

### 2. Workflow 级并发控制 — `ConcurrencyGate`

**位置**：新增 `apps/runner/src/app/concurrency.rs`

```rust
pub struct ConcurrencyGate {
    per_workflow: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    global: Arc<Semaphore>,
    config: ConcurrencyConfig,
}
```

每次 `start_workflow` / `resume_workflow` 进入执行前，必须**同时**拿到两个 permit：

**per-workflow-key permit**（限制单个 key 的并发）
- 每个 `workflow_key` 对应一个独立的 `tokio::sync::Semaphore`，许可数 = `max_concurrent_per_workflow`（默认 5）。
- 防止同一个 workflow 被高频触发时自身打满资源。

**global permit**（限制 runner 进程内的总并发）
- 所有 workflow_key 共享同一个 `Semaphore`，许可数 = `max_concurrent_global`（默认 50）。
- 防止多个不同 workflow_key 同时大量触发、总量仍超出线程池/下游承载上限。即使每个 key 自身未超限，全局兜底仍会生效。

两个 permit 均通过 RAII 持有， 闭包返回时自动释放。acquire 发生在 `spawn_blocking` **之前**（async 上下文），permit 通过 move 传入闭包。

**overflow 策略**（两层共用同一策略）：
- `Reject`：任一层 `try_acquire` 失败 → 立即返回 `AppError::Throttled`（HTTP 429）。
- `Queue`：`acquire().await` 等待槽位释放，超过 `queue_timeout_secs` → 返回 `AppError::QueueTimeout`（HTTP 503）。

**与 `RunRegistry` 的关系**：`ConcurrencyGate` 只负责准入控制；`RunRegistry` 继续负责 run_id 生命周期和终止信号，两者职责不重叠。

---

### 3. 配置结构

新增配置项（融入现有 runner config）：

```toml
[concurrency]
max_global = 50               # 全局并发上限
queue_timeout_secs = 30       # Queue 策略等待超时，0 = 永久等待
overflow_policy = "queue"     # "queue" | "reject"

[concurrency.per_workflow]
default_max = 5               # 默认每 workflow_key 并发上限
# 按 key 覆盖
"warehouse-sorting" = 10
```

---

### 4. 改动点清单

#### 新增文件

| 文件 | 内容 |
|------|------|
| `apps/runner/src/app/concurrency.rs` | `ConcurrencyGate` + `ConcurrencyConfig` |

#### 修改文件

| 文件 | 修改内容 |
|------|----------|
| `apps/runner/src/app/app.rs` | `WorkflowApp` 持有 `Arc<ConcurrencyGate>`；`start_workflow` / `resume_workflow` 在 `spawn_blocking` 前调用 `gate.acquire(workflow_key)` |
| `apps/runner/src/config.rs`（或对应配置文件） | 增加 `ConcurrencyConfig` 字段的解析 |
| HTTP 层 error mapping | `AppError::Throttled` → HTTP 429，`AppError::QueueTimeout` → HTTP 503 |

---

### 5. 对齐 `RunRegistry` — 活跃数查询

在 `RunRegistryState` 增加 `active_count_by_workflow: HashMap<String, usize>`，用于：
- 监控 API 返回当前每 workflow_key 的活跃数（可选）
- 背压指标上报

---

## 实施顺序

```
Step 1  新增 ConcurrencyConfig 及配置解析
Step 2  实现 ConcurrencyGate（含 Semaphore + OverflowPolicy）
Step 3  接入 start_workflow / resume_workflow
Step 4  HTTP error mapping（Throttled 429 / QueueTimeout 503）
Step 5  单元测试：
        - 并发超限时 Reject 返回错误
        - 并发超限时 Queue 正确等待并释放
```

---

## 风险与注意事项

- **`spawn_blocking` 与 `async acquire` 的边界**：`acquire().await` 必须在 async 上下文中调用（`start_workflow` 本身是 `async fn`），`spawn_blocking` 内部是同步的，不能在 blocking 线程里 `.await`。因此 acquire 在 `spawn_blocking` **之前**完成，permit 通过 move 语义传入闭包，闭包返回时自动 drop。
- **Resume 路径**：`resume_workflow` 同样需要经过 `ConcurrencyGate`，避免大量 resume 同时冲击。
- **配置热更新**（可选，二期）：通过 `ArcSwap<ConcurrencyConfig>` 支持不重启更新限流参数。
