# 仓储/物流分拣作业编排平台技术评审

## 1. 文档信息

| 项目 | 内容 |
| --- | --- |
| 文档名称 | 仓储/物流分拣作业编排平台技术评审 |
| 文档版本 | V1.0 |
| 文档日期 | 2026-03-30 |
| 评审目标 | 在 MVP 可交付前提下，为平台化演进给出可落地的技术架构与边界建议 |
| 当前倾向方案 | Vue Flow + 自研 DSL Runner（Rust） |
| 评审定位 | 技术路线评审，不替代详细设计与实施计划 |

## 2. 评审结论

`Vue Flow + 自研 DSL Runner（Rust）` 是一条可行路线，且对本项目比直接引入重型 BPM 或通用低代码引擎更贴合。

但该路线成立的前提不是“前端拖图 + 后端执行任意 JS”，而是：

- 前端画布与后端可执行定义分离
- Runner 按统一节点协议执行
- 运行时以“可恢复流程实例状态机”建模
- 副作用通过受控连接器、任务中心和动作节点统一收口
- 首版即具备版本、幂等、等待恢复、审计和可观测能力

一句话概括：

`前台可视化先轻量，后台执行内核先做对。`

## 3. 方案对比

### 3.1 推荐方案

`Vue Flow + JSON DSL + Node.js Runner + 受控扩展点`

优点：

- 与仓储分拣强业务语义更贴合
- 首版交付速度较快
- 可围绕任务、回调、人工接管、设备调用定制执行模型
- 后续可逐步平台化，而不是一次性引入过重引擎

缺点：

- 需要自建版本管理、状态机、等待恢复、幂等、审计能力
- 架构边界如果不提前收好，容易滑向脚本托管平台

### 3.2 不推荐作为平台主路线的方案

`Vue Flow + 任意 JS 脚本 Runner`

优点：

- PoC 最快

缺点：

- 规则、外部调用、数据库写入都容易落入脚本
- 流程不可治理、不可审计、难以排障
- 多租户与多版本能力会快速失控

### 3.3 可选但偏重的方案

`前端画布 + 标准工作流引擎适配层`

优点：

- 运行时能力成熟
- 长流程、暂停恢复、定时器、补偿能力较强

缺点：

- 与仓储分拣的业务语义并不完全贴合
- 接入和适配成本高
- 容易拖慢 MVP 交付节奏

## 4. 推荐设计原则

- 配置优先，扩展兜底
- 定义与执行分离
- 副作用统一收口
- 异步与等待是一等公民
- 流程负责编排，规则中心负责策略判断
- 运行实例绑定固定版本，发布不影响在途实例
- 首版就要支持可观测、可审计、可人工接管

## 5. DSL 设计建议

不建议直接持久化 Vue Flow 原始 `nodes/edges` 作为运行时输入。建议在发布时编译成稳定的后端 `Workflow Definition JSON`。

### 5.1 推荐结构

- `meta`
- `trigger`
- `inputSchema`
- `nodes`
- `transitions`
- `policies`

### 5.2 字段说明

`meta`

- 流程编码
- 名称
- 版本
- 作用域：租户、客户、仓
- 状态
- 发布人
- 发布时间

`trigger`

- 触发类型：`webhook`、`manual`、`event`
- 触发路径或事件名
- 响应模式：`sync`、`async_ack`

`nodes`

- `id`
- `type`
- `name`
- `config`
- `inputMapping`
- `outputMapping`
- `timeout`
- `retryPolicy`
- `onError`
- `annotations`

`transitions`

- `from`
- `to`
- `condition`
- `priority`
- `label`
- `branchType`

`policies`

- 流程级超时
- 默认重试策略
- 幂等策略
- 审计级别
- 数据保留策略
- 是否允许人工重跑

### 5.3 DSL 示例

```json
{
  "meta": {
    "key": "sorting-main-flow",
    "version": 3,
    "scope": {
      "tenant": "tenant-a",
      "warehouse": "WH-1"
    }
  },
  "trigger": {
    "type": "webhook",
    "path": "/flows/order/inbound",
    "responseMode": "async_ack"
  },
  "inputSchema": {
    "type": "object"
  },
  "nodes": [
    {
      "id": "start_1",
      "type": "start",
      "name": "Start"
    },
    {
      "id": "fetch_order",
      "type": "fetch",
      "name": "查询订单",
      "config": {
        "connector": "oms.getOrder"
      },
      "inputMapping": {
        "orderNo": "{{trigger.body.orderNo}}"
      },
      "timeout": 3000
    },
    {
      "id": "route_switch",
      "type": "switch",
      "name": "业务分流",
      "config": {
        "expression": "{{state.order.bizType}}"
      }
    }
  ],
  "transitions": [
    {
      "from": "start_1",
      "to": "fetch_order"
    },
    {
      "from": "fetch_order",
      "to": "route_switch"
    }
  ],
  "policies": {
    "idempotency": {
      "key": "{{trigger.headers.requestId}}"
    }
  }
}
```

### 5.4 强约束建议

定义层不要内嵌大段任意 JS。定义里最多只允许引用：

- 表达式
- 命名 connector
- 命名 code handler
- 命名 sub-workflow

## 6. 节点协议设计建议

节点必须遵循统一执行契约，而不是各自定义输入输出。

### 6.1 推荐执行上下文

```ts
type NodeExecutionContext = {
  runId: string
  nodeId: string
  workflowKey: string
  workflowVersion: number
  trigger: any
  input: any
  state: Record<string, any>
  env: {
    tenantId: string
    warehouseId?: string
    operatorId?: string
  }
}
```

### 6.2 推荐执行结果

```ts
type NodeExecutionResult = {
  status: "success" | "waiting" | "failed" | "skipped"
  output?: any
  statePatch?: Record<string, any>
  nextSignal?: {
    type: "webhook_response" | "external_callback" | "task_created"
    payload?: any
  }
  error?: {
    code: string
    message: string
    retryable: boolean
    details?: any
  }
}
```

### 6.3 错误模型建议

- `business_error`
- `technical_error`
- `waiting_state`

### 6.4 节点级治理字段

- `timeoutMs`
- `retryPolicy`
- `idempotencyKey`
- `onError`
- `auditLevel`
- `maskFields`
- `tags`

## 7. 运行时状态模型建议

Runner 应被建模为“可恢复流程实例状态机”，而不是顺序脚本执行器。

### 7.1 推荐核心对象

`workflow_definition`

- 发布态定义
- 固定版本
- 范围信息
- 校验摘要

`workflow_run`

- 一个业务实例对应一次流程运行
- 持有流程版本、业务主键、实例状态、当前节点

`workflow_run_state`

- 持有流程实例共享状态
- 不与主表混存

`workflow_node_execution`

- 每次节点执行一条记录
- 持有输入快照、输出快照、状态补丁、错误信息、耗时

`workflow_waiting_event`

- 承接等待回调、人工确认、设备完成等异步等待

### 7.2 推荐状态域划分

- `trigger/input`
- `runtime state`
- `execution snapshots`

### 7.3 幂等建议

`run 级幂等`

- 防止重复创建流程实例

`node 级幂等`

- 防止副作用节点重复执行

### 7.4 人工介入建议

后台不建议只提供一个统一的“重新执行”按钮。建议区分：

- 重试
- 跳过
- 转人工处理
- 补录回执

## 8. 节点分层与 MVP 节点清单

### 8.1 推荐节点分层

`流程控制节点`

- `Start`
- `End`
- `If/Else`
- `Switch`
- `Sub-Workflow`

`数据处理节点`

- `Fetch`
- `SetState`
- `Code`

`副作用节点`

- `Action/Command`

`等待/异步节点`

- `Wait`

`任务节点`

- `Task`

### 8.2 对当前候选节点的评审意见

`Start`

- 保留

`End`

- 保留

`Webhook`

- 建议拆成 `Webhook Trigger` 与 `Respond`

`Code`

- 可保留，但必须受限，只允许纯函数式数据处理

`Sub-Workflow`

- 强烈建议保留

`If/Else`

- 保留

`Switch`

- 保留

`While`

- 首版不建议对业务用户开放，或仅提供受限版本

`Fetch`

- 保留

`Set/Get State`

- 建议重构为 `SetState`，`GetState` 尽量通过表达式直接读取

`Database`

- 不建议直接以通用数据库节点形态面向业务开放
- 对外宜封装为命名业务动作或统一 `Action/Command`

### 8.3 推荐首版开放节点

- `Start`
- `End`
- `Webhook Trigger`
- `Respond`
- `Fetch`
- `SetState`
- `If/Else`
- `Switch`
- `Sub-Workflow`
- `Shell`
- `Wait`
- `Task`

### 8.4 推荐首版默认隐藏或仅高级用户开放

- `Code`
- `While`

## 9. 关键风险与规避建议

### 9.1 Vue Flow 承担过多运行语义

风险：

- 前端耦合运行时
- 定义难以演进

规避：

- 前后端定义分离
- 发布态以后端编译产物为准

### 9.2 Code 节点失控

风险：

- 平台沦为脚本托管系统

规避：

- 仅允许纯函数式处理
- 禁止直接访问网络、文件系统、数据库

### 9.3 状态模型膨胀

风险：

- 状态结构不可治理
- 重试和恢复逻辑复杂

规避：

- 状态命名空间化
- 节点快照与共享状态分离

### 9.4 缺失等待模型

风险：

- 回调恢复困难
- 现场排障成本高

规避：

- 首版引入 `waiting_event`
- 所有回调都能关联到 `run_id + node_id + execution_id`

### 9.5 副作用缺少幂等

风险：

- 重复创建设备任务
- 重复打印或贴码

规避：

- 所有副作用节点支持 node-level 幂等键

### 9.6 Database 节点暴露过底层

风险：

- 强耦合业务表
- 多租户安全风险

规避：

- 对外封装为命名动作

### 9.7 多租户、多版本治理做晚

风险：

- 模板派生、回滚、灰度发布成本高

规避：

- 首版就引入 `workflow_key + version + scope`

### 9.8 规则中心与流程引擎边界不清

风险：

- 流程图变成规则脚本图

规避：

- 流程负责编排
- 规则中心负责策略判断

### 9.9 可观测能力后补

风险：

- 现场联调困难
- 无法快速定位阻塞点

规避：

- 首版即建设实例时间线、节点耗时、错误码、trace 关联

## 10. 推荐总体架构蓝图

### 10.1 模块划分

前端：

- 流程编排台
- 运营控制台
- 任务工作台
- 规则与连接器管理台

后端：

- `Workflow Definition Service`
- `Workflow Engine Service`
- `Task Service`
- `Connector Gateway`
- `Rule Service`
- `Observability & Audit`

基础设施：

- `PostgreSQL`
- `Redis`
- `Message Bus`
- `Object Storage` 可选

### 10.2 Engine 内部建议模块

- `Definition Loader`
- `Execution Orchestrator`
- `Transition Resolver`
- `Executor Registry`
- `State Manager`
- `Waiting Manager`
- `Idempotency Guard`

### 10.3 PlantUML 架构图

独立文件见：

- [warehouse-sorting-workflow-architecture.puml](/Users/zhangjunqing/git-hy/ses-flow/docs/warehouse-sorting-workflow-architecture.puml)

## 11. 关键时序建议

核心链路建议采用：

`实例创建 -> 节点推进 -> 等待事件 -> 外部回调 -> 恢复执行 -> 流程结束`

### 11.1 PlantUML 时序图

独立文件见：

- [warehouse-sorting-workflow-sequence.puml](/Users/zhangjunqing/git-hy/ses-flow/docs/warehouse-sorting-workflow-sequence.puml)

## 12. MVP 分阶段落地路线图

### 阶段一：定义与执行最小闭环

目标：

- 完成流程定义模型
- 支持 `Webhook Trigger -> Fetch -> Switch -> Action -> End`
- 支持发布态版本绑定

范围：

- 流程定义管理
- DSL 编译与校验
- Engine 最小执行器
- 实例与节点执行记录

不做：

- 高级节点
- 可视化时间线
- 复杂人工干预

### 阶段二：等待恢复与可观测

目标：

- 支持 `Wait`
- 支持回调恢复
- 支持实例时间线

范围：

- `workflow_waiting_event`
- 回调关联机制
- 关键节点 trace
- 基础告警与审计

### 阶段三：任务中心与人工接管

目标：

- 支持人工/PDA/设备统一任务模型
- 支持人工重试、跳过、补录回执

范围：

- `Task Service`
- 岗位工作台对接
- 人工介入能力

### 阶段四：模板复用与平台治理

目标：

- 支持模板派生
- 支持客户/仓级差异覆盖
- 支持灰度与回滚

范围：

- 模板层级模型
- 范围化发布
- 变更审计
- 发布影响分析

## 13. 推荐首版技术选型

### 13.1 前端

- Vue 3
- Vue Flow
- Pinia
- TypeScript

### 13.2 后端

- Rust

### 13.3 存储与基础设施

- PostgreSQL
- Redis
- Kafka / RabbitMQ / RocketMQ 任选其一，按团队现有栈优先

## 14. 最终建议

这条路线可行，但必须避免把自研 Runner 做成脚本执行器。

真正值得优先投入的不是“节点做得多快”，而是以下四件事：

- 稳定的定义层
- 可恢复的运行时状态机
- 副作用统一收口
- 可观测与人工接管能力

如果以上边界从首版就建立好，`Vue Flow + Rust 自研 DSL Runner` 完全可以作为仓储/物流分拣作业编排平台的合理起点，并能支撑后续向多租户、多版本、多模板的平台化方向演进。
