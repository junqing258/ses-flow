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

### 路径 B：原子型节点（插件节点，默认 HTTP，后续支持本地进程）

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

路径 B 统一使用一个插件请求/响应模型，不同 transport 只影响传输方式，不影响语义：

```jsonc
// runner → plugin（HTTP body 或 stdin）
{
  "nodeId": "node-123",
  "config": { ... },          // 面板配置，来自 configSchema
  "inputMapping": { ... },
  "context": {
    "runId": "run-abc",
    "workflowKey": "wf-001",
    "input": { ... },
    "state": { ... },
    "env": { ... }
  }
}

// plugin → runner（HTTP response body 或 stdout）
{
  "output": { ... },          // 节点输出，进入后续 outputMapping
  "logs": [
    { "level": "info", "message": "扫描成功", "ts": 1234567890 }
  ],
  "error": null
}
```

#### HTTP 插件标准接口

首期不做 runner 侧的“自动扫网段发现服务”，而采用**注册中心 + 标准插件 API**的模式。每个 HTTP 插件至少暴露 3 个固定接口：

- `GET /descriptor`：返回插件自描述，用于注册校验、版本比对和管理台展示
- `GET /health`：健康检查，用于平台接入校验和告警
- `POST /execute`：主执行入口，runner 调用该接口执行业务逻辑

`GET /descriptor` 返回示例：

```json
{
  "id": "barcode_scan",
  "runnerType": "plugin:barcode_scan",
  "version": "1.0.0",
  "displayName": "条码扫描",
  "transport": "http",
  "configSchema": { ... },
  "timeoutMs": 5000
}
```

`POST /execute` 返回示例：

```json
{
  "output": { "barcode": "BC-001" },
  "logs": [
    { "level": "info", "message": "scan completed" }
  ],
  "error": null
}
```

#### 服务发现策略

路径 B 的 HTTP 插件需要标准化接口，但**首期不建议让 runner 自己去做网络级自动发现**（如扫端口、扫网段、直连 Consul/etcd）。推荐策略是：

1. 插件服务实现标准接口：`/descriptor`、`/health`、`/execute`
2. 运维/实施在平台注册插件 `baseUrl`
3. 平台调用 `GET /descriptor` 完成协议校验并写入 registry
4. runner 执行时只按 `runnerType` 查 registry，再调用对应插件

这样可以避免把服务发现、网络治理、执行逻辑耦合到 runner 内部。

#### 后续支持：本地进程插件

对于需要更强隔离、设备侧本机调用、或现场不方便部署 HTTP 服务的场景，后续支持：

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

    async fn execute(&self, node: &NodeDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let input = build_plugin_input(node, ctx);
        match self.transport {
            PluginTransport::Http { endpoint } => call_http_plugin(endpoint, input).await,
            PluginTransport::Process { binary } => call_process_plugin(binary, input).await,
        }
    }
}
```

| 对比项 | 路径 A（组合型） | 路径 B（插件型） |
|---|---|---|
| 执行逻辑 | 复用子流程 | 独立二进制，任意语言 |
| 前端改动 | 零 | 零 |
| Runner 核心改动 | 零 | 仅一次性增加 `PluginExecutor` |
| 扩展者工作量 | 写 JSON descriptor | 首期写 HTTP 插件服务 + descriptor；后续可写本地可执行包 |
| 调试方式 | 看子流程 | 首期最友好：HTTP 联调；后续可选本地进程 |
| 进程隔离 | 无 | process 模式天然进程隔离；HTTP 模式天然服务隔离 |
| 适用场景 | 现有能力的业务封装 | 全新平台能力 |

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
  transport?: "builtin" | "http" | "process"; // 路径 B 使用 http/process，内置节点默认 builtin
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
    node_registry.go          // 注册中心，维护全量 descriptor
    descriptors/
      builtin/
        sub_workflow.go       // 现有节点迁移
        fetch_order.go
        assign_task.go
      biz/
        pick_task.json        // 路径 A：纯 JSON descriptor
        sort_task.json
      custom/
        barcode_scan.json     // 路径 B：HTTP / process 插件 descriptor
    plugin_registry.go        // 维护 runnerType -> endpoint/binary 映射
  api/
    GET /api/node-descriptors              // 按 token 权限过滤返回
    GET /api/node-descriptors/:id/versions
    POST /api/plugin-registrations         // 注册 HTTP 插件 baseUrl，回拉 /descriptor 校验
```

---

## 阶段二：前端 NodeRegistry 替代硬编码

### 2.1 新增 `nodeRegistry.ts`

```typescript
// apps/frontend/src/features/workflow/nodeRegistry.ts

class NodeRegistry {
  private byId    = new Map<string, FrontendNodeDescriptor>();
  private byPalette  = new Map<string, FrontendNodeDescriptor>();
  private byRunner   = new Map<string, FrontendNodeDescriptor>();

  async load(apiUrl: string) {
    const raw: NodeDescriptor[] = await fetch(apiUrl).then(r => r.json());
    for (const d of raw) {
      const fd = { ...d, paletteId: `palette-${d.id}`, panelTemplate: schemaToPanel(d.configSchema) };
      this.byId.set(d.id, fd);
      this.byPalette.set(fd.paletteId, fd);
      this.byRunner.set(d.runnerType, fd);   // 注意：多个 descriptor 可共享同一 runnerType（路径 A）
    }
  }

  getByPaletteId(paletteId: string)  { return this.byPalette.get(paletteId); }
  getByRunnerType(runnerType: string) { return this.byRunner.get(runnerType); }
  getByCategory(category: string)    { return [...this.byId.values()].filter(d => d.category === category); }
}

export const nodeRegistry = new NodeRegistry();
```

### 2.2 改造现有硬编码点

| 文件 | 改造前 | 改造后 |
|---|---|---|
| `model.ts:642` | `INITIAL_WORKFLOW_PANELS` 静态对象 | `nodeRegistry.getByPaletteId(id)?.panelTemplate` |
| `model.ts:1283` | switch/case 工厂函数 | registry lookup + 通用 draft 构造 |
| `runner.ts:405` | 硬编码 kind→type 表 | `nodeRegistry.getByPaletteId(id)?.runnerType` |
| `runner.ts:461` | 每类节点独立序列化 | 通用 schema-based config 序列化 |
| `import.ts:33` | 硬编码 type→paletteId 表 | `nodeRegistry.getByRunnerType(type)?.paletteId` |

**迁移策略**：`INITIAL_WORKFLOW_PANELS` 保留为 fallback，registry 未就绪时降级，渐进迁移不破坏现有功能。

---

## 阶段三：版本管理与权限控制

- `status: "beta"` → 前端面板显示 Beta 标签
- `status: "deprecated"` → 阻止新建，存量节点显示弃用提示
- 旧工作流保存 `version` 字段，import 时按版本 schema 解析，向后兼容
- `GET /api/node-descriptors` 按 token 过滤 `requiredPermissions`，前端无需维护权限列表

---

## 实施路径（约 6–8 周）

```
Week 1–2  Phase 1  定义协议 + 后端注册中心 + API 接口
                   现有节点迁移为 descriptor（builtin/）
Week 3–4  Phase 2  前端 nodeRegistry.ts + schemaToPanel 转换器
                   保留 INITIAL_WORKFLOW_PANELS 作为 fallback
Week 5    Phase 3  替换 5 个硬编码文件，跑通现有全部节点
Week 6    Phase 4  版本字段 + 权限过滤接入
Week 7+   持续     路径 A（JSON）/ 路径 B（代码）新增节点，前端零改动
```

---

## 关键收益

- **路径 A 新增节点**：写一个 JSON descriptor，零代码
- **路径 B 新增节点**：首期接入 HTTP 插件服务 + descriptor，后续可演进为本地进程插件
- **面板表单**：configSchema + x-* 声明，无需写 Vue 组件
- **向后兼容**：version 字段保证旧工作流可正确导入
- **权限治理**：节点可见性由后端按 token 过滤，前端无感
