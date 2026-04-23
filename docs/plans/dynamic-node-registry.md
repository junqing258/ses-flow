# 动态业务节点注册机制实施方案

## 背景与目标

当前节点架构完全静态硬编码：新增一个业务节点需改动 4+ 个核心文件（`model.ts`、`runner.ts`、`import.ts` 等），触碰主流程逻辑，研发成本高、风险大。

**目标**：让新增业务能力不必每次都硬编码进主流程编辑器，通过元数据驱动实现平台能力扩展。

---

## 现状分析

### 现有节点的完整身份映射（以 sub-workflow 为例）

| 层级 | 值 |
|---|---|
| 面板列表 id | `"palette-subflow"` |
| 画布 `kind` | `"sub-workflow"` |
| VueFlow 渲染类型 | `"workflow-card"` |
| 面板模板 key | `"sub_workflow"` |
| Runner wire type | `"sub_workflow"` |

一个节点当前涉及 5 层映射，分散在 5 个文件：

| 文件 | 位置 | 硬编码内容 |
|---|---|---|
| `model.ts` | L642–1220 `INITIAL_WORKFLOW_PANELS` | 所有节点的面板字段静态声明 |
| `model.ts` | L1283–1625 `createWorkflowNodeDraft` | switch/case 枚举每种节点草稿 |
| `runner.ts` | L405–459 `extractNodeType()` | kind → runnerType 映射 |
| `runner.ts` | L461–618 `buildNodeDefinition()` | 每类节点独立序列化逻辑 |
| `import.ts` | L33–64 `getPaletteIdForRunnerNodeType()` | runnerType → paletteId 反向映射 |

---

## 核心问题：新增节点走哪条路？

方案明确区分**两类**新业务节点，走不同的扩展路径：

### 路径 A：组合型节点（类子流程，零代码扩展）

**适用场景**：新业务能力可以通过**组合现有节点**实现，例如"拣货任务"= fetch + 条件 + effect 的固定编排。

**扩展方式**：JSON 配置，无需写代码。

```
新增一个"拣货任务"节点：
  ↓
后端注册表添加一条 NodeDescriptor JSON
  ↓
前端自动出现在面板、可拖拽、有配置项
  ↓
执行时，runner 将其展开为内置子流程调用（同 sub-workflow）
```

本质上是**参数化的子流程**：`runnerType = "sub_workflow"`，`config.ref` 指向一个预定义的子工作流模板。新业务节点的执行逻辑已在子工作流中定义，扩展者只需声明该节点的面板表单和默认参数。

```json
// 示例：新增"拣货任务"节点的 descriptor（纯 JSON，无需改代码）
{
  "id": "pick_task",
  "kind": "sub-workflow",
  "runnerType": "sub_workflow",
  "category": "业务节点",
  "displayName": "拣货任务",
  "status": "stable",
  "configSchema": {
    "type": "object",
    "properties": {
      "warehouseId": { "type": "string", "title": "仓库", "x-tab": "base", "x-component": "select" },
      "pickMode":    { "type": "string", "title": "拣货模式", "x-tab": "base", "x-component": "radio", "x-options": ["单品", "批次"] }
    }
  },
  "defaults": {
    "workflowRef": "tpl_pick_task_v1"
  }
}
```

### 路径 B：原子型节点（插件节点，首期 HTTP，中期 gRPC）

**适用场景**：新业务能力需要**全新的执行逻辑**，无法通过现有节点组合实现，例如调用外部硬件 API、写特定数据库、对接第三方系统。

**扩展方式**：路径 B 统一抽象为 **plugin 节点**，首期默认采用 **HTTP/JSON 插件协议**，以兼容既有软件和降低联调门槛；后续补充 **本地子进程 + stdin/stdout JSON** 协议，满足高隔离、低依赖和设备侧集成场景。

```
新增一个"条码扫描"节点：
  ↓
① 先有软件团队提供一个 HTTP 插件服务
   GET /descriptor
   GET /health
   POST /execute
  ↓
② 平台注册 NodeDescriptor（runnerType: "plugin:barcode_scan"，transport: "http"，endpoint: "http://..."）
  ↓
③ runner 内置 PluginExecutor
   匹配 runnerType 前缀 "plugin:" → 按 transport 调 HTTP 或本地进程
  ↓
前端：自动出现节点、面板字段由 configSchema 驱动，零改动
```

#### 插件调用协议

路径 B 统一使用一个插件请求/响应模型，不同 transport 只影响传输方式，不影响语义。

动态节点调用时，runner 必须透传两类链路标识：

- `runId`：工作流运行实例 id，用于定位本次执行、终止、恢复和日志归档。
- `requestId`：业务请求幂等/联查 id，优先透传 trigger payload 或上游节点上下文中的 `requestId`；首次执行、恢复执行、取消执行都必须保持同一个值。

响应的 `status` 对应 runner 内部 `NodeExecutionResult` 的三种状态（`success`/`waiting`/`failed`）：

```jsonc
// runner → plugin（HTTP body 或 stdin）
{
  "nodeId": "node-123",
  "config": { ... },
  "context": {
    "runId": "run-abc",
    "requestId": "req-001",
    "workflowKey": "wf-001",
    "input": { ... },
    "state": { ... },
    "env": { ... },
    "resumeSignal": null   // 首次执行为 null；恢复时携带外部回调的 payload
  }
}

// plugin → runner（HTTP response body 或 stdout）
{
  "status": "success" | "waiting" | "failed",  // 必填

  // status = "success"
  "output": { ... },
  "statePatch": { ... },   // 可选，更新工作流 state
  "logs": [ { "level": "info", "message": "..." } ],

  // status = "waiting" — runner 将工作流挂起，等待外部回调
  // 插件需声明等待什么事件，runner 持久化 WorkflowRunSnapshot 后暂停
  // 收到匹配回调后，runner 重新调用插件，resumeSignal 携带回调数据
  "waitSignal": {
    "type": "barcode_scanned",
    "payload": { "taskId": "t-001" }
  },

  // status = "failed"
  "error": {
    "code": "SCAN_TIMEOUT",
    "message": "扫描超时",
    "retryable": true
  }
}
```

**终止（Terminate）**

- **process 模式**：`PluginExecutor` 复用 `wait_for_process_output()` 的 `context.should_terminate()` 检测，直接 `kill()` 子进程，插件进程无需感知。
- **HTTP 模式**：runner 在收到终止信号后，主动调用插件的 `POST /cancel` 接口，让插件有机会清理资源（释放硬件锁、取消外部任务单等）；调用后取消 HTTP 请求。`/cancel` 请求体同样要携带 `runId`、`requestId`，便于插件侧做幂等清理和审计。

**恢复（Resume）**

- **process 模式**：不需要插件实现额外接口。插件声明 `status: "waiting"` 后 runner 挂起工作流；外部回调到达时，runner 重新 spawn 插件进程，`stdin` 中 `resumeSignal` 携带回调 payload，插件通过判断 `resumeSignal` 是否为 `null` 区分首次执行和恢复。
- **HTTP 模式**：外部系统调用插件的 `POST /resume`，由**插件主动通知 runner** 继续（携带回调结果），而非外部系统直接回调 runner。这样回调路由逻辑封装在插件内部，runner 无需暴露额外 webhook 端点。`/resume` 请求体必须继续透传首次执行时的 `runId`、`requestId`，确保恢复链路能和原始业务请求关联。

```rust
// process 模式插件示例
fn execute(&self, req: PluginRequest) -> PluginResponse {
    if let Some(signal) = req.context.resume_signal {
        return PluginResponse::success(signal.payload);
    }
    PluginResponse::waiting("barcode_scanned", json!({ "taskId": "t-001" }))
}
```

#### 日志约束

##### 统一日志格式

插件响应体中 `logs` 数组的每条记录，以及 runner 落库/输出时的每条结构化日志，**统一使用以下格式**：

```jsonc
{
  // ── 插件填写（plugin → runner）──────────────────────────────
  "level":   "info",               // 必填：trace | debug | info | warn | error
  "message": "等待扫码中",          // 必填：人类可读的事件描述

  "fields": {                      // 可选：与本条日志直接相关的业务字段
    "taskId":   "t-001",
    "duration": 120                // 单位 ms
  },

  // ── runner 注入（归档/转发时自动补充，插件禁止填写）────────────
  "runId":     "run-abc",          // 工作流运行实例
  "requestId": "req-001",          // 业务幂等键，贯穿首次/恢复/取消全链路
  "nodeId":    "node-123",         // 节点定位
  "traceId":   "a1b2c3d4e5f6",     // 跨服务链路 ID，来自 HTTP Header X-Trace-Id
  "timestamp": "2026-04-23T10:00:00.123Z"  // ISO 8601，runner 记录接收时间
}
```

**字段职责说明**

| 字段 | 填写方 | 说明 |
|---|---|---|
| `level` | 插件 | 见下方级别语义 |
| `message` | 插件 | 信息 |
| `fields` | 插件 | 任意 KV |
| `runId` | runner | 来自请求 `context.runId` |
| `requestId` | runner | 来自请求 `context.requestId` |
| `nodeId` | runner | 来自请求 `nodeId` |
| `traceId` | runner | 来自请求 HTTP Header `X-Trace-Id`；HTTP 插件须在响应 Header 中原样回传 |
| `timestamp` | runner | runner 收到日志的时刻，不依赖插件侧时钟 |

##### 级别语义

| 级别 | 适用场景 |
|---|---|
| `trace` | 内部循环、逐帧/逐条数据处理，生产默认不输出 |
| `debug` | 关键中间状态，联调时开启 |
| `info` | 节点开始、结束、挂起等关键生命周期事件 |
| `warn` | 可降级处理的异常（重试、超时降级等） |
| `error` | 导致 `status: "failed"` 的根因；必须与响应体 `error.code` 对应 |

`status: "failed"` 时，`error.message` 与 `logs` 末尾的 `error` 级别日志 `message` **语义必须一致**，runner 以此作为 UI 展示和采集的唯一来源。


首期不做 runner 侧的"自动扫网段发现服务"，而采用**注册中心 + 标准插件 API**的模式。每个 HTTP 插件暴露 **5 个固定接口**：

| 接口 | 调用方 | 说明 |
|---|---|---|
| `GET /descriptor` | 平台注册时 | 返回插件自描述，用于协议校验和管理台展示 |
| `GET /health` | 平台 / runner | 健康检查 |
| `POST /execute` | runner | 主执行入口，首次执行和恢复执行复用此接口 |
| `POST /cancel` | runner | 工作流被终止时调用，插件清理资源 |
| `POST /resume` | 外部系统 | 外部回调入口，插件收到后主动通知 runner 继续 |

`GET /descriptor` 返回示例：

```json
{
  "id": "barcode_scan",
  "runnerType": "plugin:barcode_scan",
  "version": "1.0.0",
  "displayName": "条码扫描",
  "transport": "http",
  "configSchema": { ... },
  "timeoutMs": 5000,
  "supportsCancel": true,
  "supportsResume": true
}
```

`POST /execute` 响应示例（挂起等待外部回调）：

```json
{
  "status": "waiting",
  "waitSignal": {
    "type": "barcode_scanned",
    "payload": { "taskId": "t-001" }
  },
  "logs": [{ "level": "info", "message": "等待扫码中" }]
}
```

`POST /cancel` 请求体：

```json
{
  "runId": "run-abc",
  "requestId": "req-001",
  "nodeId": "node-123",
  "reason": "workflow_terminated"
}
```

`POST /resume` 请求体（外部系统 → 插件）：

```json
{
  "runId": "run-abc",
  "requestId": "req-001",
  "nodeId": "node-123",
  "signal": {
    "type": "barcode_scanned",
    "payload": { "barcode": "1234567890" }
  }
}
```

插件收到 `/resume` 后，向 runner 回调（携带执行结果），runner 继续推进工作流。`supportsCancel` / `supportsResume` 为 `false` 的插件不需要实现对应接口，runner 对 cancel 降级为直接中断，对 resume 不允许该节点挂起。对动态节点的任一次调用（`/execute`、`/cancel`、`/resume` 或 process `stdin`）都要求至少带上同一组 `runId` + `requestId`。

#### 服务发现策略

路径 B 的 HTTP 插件需要标准化接口，但**首期不建议让 runner 自己去做网络级自动发现**（如扫端口、扫网段、直连 Consul/etcd）。推荐策略是：

1. 插件服务实现标准接口：`/descriptor`、`/health`、`/execute`
2. 运维/实施在平台注册插件 `baseUrl`
3. 平台调用 `GET /descriptor` 完成协议校验并写入 registry
4. runner 执行时只按 `runnerType` 查 registry，再调用对应插件

这样可以避免把服务发现、网络治理、执行逻辑耦合到 runner 内部。

#### transport 分层策略

不同场景对调试体验、运行效率、内存开销、部署条件的要求不同，plugin 通信方案分三层逐步演进：

| transport | 单次调用延迟 | 内存开销 | 调试体验 | 隔离方式 | 推荐场景 |
|---|---|---|---|---|---|
| `http` | ~1–5ms | 每请求独立连接（keep-alive 复用后~低）；JSON 序列化拷贝一份 | 最好（curl/Postman 直测） | 进程/网络隔离 | 跨机器、已有服务、首期默认 |
| `process` (stdin/stdout) | ~5–20ms（含 fork）| 每次调用 fork 一个新进程，内存独立但开销最高（~数 MB/次）| 良好（直接跑二进制） | 进程隔离，崩溃不污染 runner | 设备侧、现场无 HTTP 服务 |
| `grpc` (Unix socket) | <0.5ms | 长驻进程复用，Protobuf 序列化比 JSON 节省 30–50%；连接常驻，无 fork 开销 | 良好（grpcurl）| 进程隔离 | 同机高频调用、需流式日志 |
| `wasm` (wasmtime) | <0.1ms（同进程）| 与 runner 共享进程堆，线性内存按需增长；无 fork/序列化拷贝，内存效率最高 | 一般（需专用工具） | 沙箱隔离（内存隔离但同进程）| 远期：平台内置扩展点 |
| 动态库 `.so` | <0.05ms（函数调用）| 与 runner 完全共享内存，无拷贝 | 差（链接问题难排查）| 无隔离，崩溃即 runner 崩溃 | **不推荐**，ABI 不稳定 |

**内存开销排序（从高到低）**：`process` > `http` > `grpc` > `wasm` > `.so`

**首期实现：`http`**，覆盖跨机器、已有服务、联调调试等主要场景。

**中期补充：`grpc` over Unix socket**，作为 `process` 的高性能替代：
- runner 侧用 `tonic`（Rust gRPC 库），插件侧实现同一份 `.proto`
- Unix socket 消除网络栈，延迟与 stdin/stdout 相当，但支持**流式推送日志**（不必等执行完才返回 logs）
- `cancel` 和 `resume` 天然是独立 RPC，比 HTTP 接口更清晰
- 调试：`grpcurl -plaintext -unix /tmp/ses-plugin-barcode.sock describe`

```protobuf
// plugin.proto（所有插件共用）
service Plugin {
  rpc Execute (PluginRequest)  returns (PluginResponse);
  rpc Cancel  (CancelRequest)  returns (CancelResponse);
}
// Resume 由外部系统调插件自己的 /resume 端点，不走 runner→插件方向
```

**远期参考：`wasm`**，用 wasmtime 在 runner 进程内执行插件，同进程调用无 fork/网络开销，沙箱隔离优于动态库 `.so`。适合平台能力稳定、对执行延迟极敏感的内置扩展点，首期不建议引入（工具链对业务团队要求高）。

**不推荐：动态库 `.so`**。ABI 不稳定，Rust 版本/编译器变化即可导致崩溃，调试极难，收益不抵风险。

#### 本地进程插件（process transport，中后期）

对于设备侧本机调用、或现场不方便部署 HTTP/gRPC 服务的场景，中后期支持：

- `transport: "process"`
- runner 通过 `Command::new(binary)` 直接拉起插件进程
- 通过 `stdin/stdout JSON` 传输与上文完全相同的请求/响应结构

```json
{
  "id": "barcode_scan_local",
  "runnerType": "plugin:barcode_scan_local",
  "transport": "process",
  "binary": "/usr/local/lib/ses-runner/plugins/barcode-scan-executor",
  "category": "业务节点",
  "configSchema": { ... }
}
```

#### PluginExecutor（runner 侧唯一新增代码）

```rust
// apps/runner/src/core/executors/plugin_executor.rs

pub struct PluginExecutor {
    transport: PluginTransport,
}

impl NodeExecutor for PluginExecutor {
    fn node_type(&self) -> &NodeType { ... }

    fn execute(&self, node: &NodeDefinition, ctx: &NodeExecutionContext<'_>) -> Result<NodeExecutionResult, RunnerError> {
        let input = build_plugin_input(node, ctx);
        match &self.transport {
            PluginTransport::Http { endpoint } => call_http_plugin(endpoint, &input, ctx),
            PluginTransport::Process { binary } => call_process_plugin(binary, &input, ctx),
            PluginTransport::Grpc { socket } => call_grpc_plugin(socket, &input, ctx),  // 中期
        }
    }
}
```

| 对比项 | `http` | `process` | `grpc`（Unix socket） |
|---|---|---|---|
| 延迟 | ~1–5ms | ~5–20ms | <0.5ms |
| 调试 | 最友好 | 友好 | 良好（grpcurl） |
| 流式日志 | 需轮询 | 不支持 | 原生支持 |
| cancel/resume | `POST /cancel` / `POST /resume` | kill / 重新 spawn | 独立 RPC |
| 实现阶段 | 首期 | 中后期 | 中期 |


---

## 目标架构

```
                   NodeDescriptor（统一协议）
                          ↓ 后端注册
          ┌───────────────┴───────────────┐
          │     GET /api/node-descriptors  │
          └───────────────────────────────┘
                          ↓ 前端加载
                    NodeRegistry.ts
          ┌───────────────┬───────────────┐
          ↓               ↓               ↓
     面板渲染          执行器路由       版本/权限
  (schemaToPanel)   (runner.ts通用)   (status/permissions)
```

---

## 阶段一：定义 NodeDescriptor 协议

### 1.1 Schema 定义

```typescript
interface NodeDescriptor {
  id: string;                     // 唯一标识，如 "pick_task"
  kind: string;                   // 前端渲染 kind，如 "sub-workflow" | "effect"
  runnerType: string;             // runner wire type，如 "sub_workflow" | "plugin:barcode_scan"
  version: string;                // 语义化版本 "1.0.0"
  category: string;               // "业务节点" | "控制流" | "触发器"
  displayName: string;
  description?: string;
  icon?: string;
  status: "stable" | "beta" | "deprecated";
  requiredPermissions?: string[];
  transport?: "builtin" | "http" | "grpc" | "process"; // 首期 http；中期 grpc；中后期 process；内置节点默认 builtin
  endpoint?: string;              // transport=http 时必填
  binary?: string;                // transport=process 时必填
  timeoutMs?: number;

  configSchema: JSONSchema7;       // 面板表单 + x-* UI 扩展
  defaults?: Record<string, unknown>; // 节点创建时的默认配置值
  inputMappingSchema?: JSONSchema7;
  outputMappingSchema?: JSONSchema7;
}
```

`configSchema` 中用 `x-*` 扩展描述 UI，无需写 Vue 组件：

```json
{
  "properties": {
    "warehouseId": {
      "type": "string",
      "title": "仓库",
      "x-tab": "base",
      "x-component": "select",
      "x-data-source": "api:/warehouses"
    }
  }
}
```

### 1.2 后端注册中心

```
executor/
  registry/
    node_registry.rs         // 注册中心，维护全量 descriptor
    descriptors/
      builtin/
        sub_workflow.rs      // 现有节点迁移
        fetch_order.rs
        assign_task.rs
      biz/
        pick_task.json        // 路径 A：纯 JSON descriptor
        sort_task.json
      custom/
        barcode_scan.json     // 路径 B：HTTP / process 插件 descriptor
    plugin_registry.rs       // 维护 runnerType -> endpoint/binary 映射
  api/
    GET /api/node-descriptors              // 按 token 权限过滤返回
    GET /api/node-descriptors/:id/versions
    POST /api/plugin-registrations         // 注册 HTTP 插件 baseUrl，回拉 /descriptor 校验
```

---


## 后续：版本管理与权限控制

- `status: "beta"` → 前端面板显示 Beta 标签
- `status: "deprecated"` → 阻止新建，存量节点显示弃用提示
- 旧工作流保存 `version` 字段，import 时按版本 schema 解析，向后兼容
- `GET /api/node-descriptors` 按 token 过滤 `requiredPermissions`，前端无需维护权限列表

---

## 关键收益

- **路径 A 新增节点**：写一个 JSON descriptor，零代码
- **路径 B 新增节点**：首期接入 HTTP 插件服务 + descriptor，中期演进为 gRPC，中后期支持本地进程插件
- **面板表单**：configSchema + x-* 声明，无需写 Vue 组件
- **向后兼容**：version 字段保证旧工作流可正确导入
- **权限治理**：节点可见性由后端按 token 过滤，前端无感
