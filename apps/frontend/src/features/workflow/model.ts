import { Position, type Edge, type Node } from "@vue-flow/core";
import {
  Activity,
  Clock3,
  Database,
  GitBranch,
  Hand,
  Info,
  ListTodo,
  Lock,
  Maximize2,
  MoreHorizontal,
  MousePointer2,
  Play,
  SendHorizontal,
  ShieldCheck,
  Webhook,
  Zap,
} from "lucide-vue-next";

export const WORKFLOW_ICON_MAP = {
  activity: Activity,
  clock: Clock3,
  database: Database,
  gitBranch: GitBranch,
  hand: Hand,
  info: Info,
  listTodo: ListTodo,
  lock: Lock,
  maximize: Maximize2,
  more: MoreHorizontal,
  mousePointer: MousePointer2,
  play: Play,
  send: SendHorizontal,
  shield: ShieldCheck,
  webhook: Webhook,
  zap: Zap,
} as const;

export type WorkflowIconKey = keyof typeof WORKFLOW_ICON_MAP;
export type WorkflowNodeKind =
  | "start"
  | "trigger"
  | "fetch"
  | "if-else"
  | "switch"
  | "shell"
  | "effect"
  | "wait"
  | "end"
  | "branch-label";
export type WorkflowNodeType = "terminal" | "workflow-card" | "branch-chip";
export type WorkflowTabId = "base" | "mapping" | "retry" | "error";
export type WorkflowFieldType = "input" | "readonly" | "select" | "textarea";
export type WorkflowExecutionStatus =
  | "running"
  | "success"
  | "waiting"
  | "failed"
  | "skipped";

export interface WorkflowNodeData {
  active?: boolean;
  accent: string;
  executionStatus?: WorkflowExecutionStatus;
  icon: WorkflowIconKey;
  kind: WorkflowNodeKind;
  nodeKey: string;
  status?: "draft" | "published";
  subtitle?: string;
  title: string;
}

export interface WorkflowField {
  key: string;
  label: string;
  type: WorkflowFieldType;
  value: string;
}

export interface WorkflowNodePanel {
  fieldsByTab: Partial<Record<WorkflowTabId, WorkflowField[]>>;
  tabs: WorkflowTabId[];
}

export interface WorkflowPaletteItem {
  accent: string;
  icon: WorkflowIconKey;
  id: string;
  kind: WorkflowNodeKind;
  label: string;
}

export interface WorkflowPaletteCategory {
  defaultOpen: boolean;
  icon: WorkflowIconKey;
  id: string;
  items: WorkflowPaletteItem[];
  label: string;
}

export interface WorkflowNodeDraft {
  node: WorkflowFlowNode;
  panel: WorkflowNodePanel;
}

export type WorkflowFlowNode = Node<
  WorkflowNodeData,
  Record<string, never>,
  WorkflowNodeType
> & {
  data: WorkflowNodeData;
  type: WorkflowNodeType;
};

export const WORKFLOW_TAB_LABELS: Record<WorkflowTabId, string> = {
  base: "基础配置",
  mapping: "输入映射",
  retry: "重试策略",
  error: "错误处理",
};

export const WORKFLOW_EMPTY_TAB_TEXT: Record<WorkflowTabId, string> = {
  base: "当前节点暂时没有更多基础配置项。",
  error: "错误处理策略会在接入运行时后继续补充。",
  mapping: "输入映射区域预留给变量绑定和表达式配置。",
  retry: "重试策略会在接入执行引擎后和运行时规则联动。",
};

export const WORKFLOW_PALETTE_CATEGORIES: WorkflowPaletteCategory[] = [
  {
    id: "trigger",
    label: "触发器",
    icon: "zap",
    defaultOpen: true,
    items: [
      {
        id: "palette-webhook",
        kind: "trigger",
        label: "Webhook Trigger",
        icon: "webhook",
        accent: "#6366F1",
      },
      {
        id: "palette-respond",
        kind: "effect",
        label: "Respond",
        icon: "send",
        accent: "#8B5CF6",
      },
    ],
  },
  {
    id: "control",
    label: "流程控制",
    icon: "gitBranch",
    defaultOpen: true,
    items: [
      {
        id: "palette-start",
        kind: "start",
        label: "Start",
        icon: "play",
        accent: "#10B981",
      },
      {
        id: "palette-end",
        kind: "end",
        label: "End",
        icon: "shield",
        accent: "#EF4444",
      },
      // { id: "palette-if-else", kind: "if-else", label: "If / Else", icon: "gitBranch", accent: "#F97316" },
      {
        id: "palette-switch",
        kind: "switch",
        label: "Switch",
        icon: "gitBranch",
        accent: "#EC4899",
      },
      {
        id: "palette-subflow",
        kind: "trigger",
        label: "Sub-Workflow",
        icon: "webhook",
        accent: "#6366F1",
      },
    ],
  },
  {
    id: "data",
    label: "数据处理",
    icon: "database",
    defaultOpen: false,
    items: [
      {
        id: "palette-fetch",
        kind: "fetch",
        label: "Fetch",
        icon: "database",
        accent: "#3B82F6",
      },
    ],
  },
  {
    id: "effect",
    label: "副作用",
    icon: "activity",
    defaultOpen: false,
    items: [
      {
        id: "palette-shell",
        kind: "shell",
        label: "Shell",
        icon: "zap",
        accent: "#F97316",
      },
    ],
  },
  {
    id: "wait",
    label: "等待 / 异步",
    icon: "clock",
    defaultOpen: false,
    items: [
      {
        id: "palette-wait",
        kind: "wait",
        label: "Wait",
        icon: "clock",
        accent: "#F59E0B",
      },
    ],
  },
  {
    id: "task",
    label: "任务",
    icon: "listTodo",
    defaultOpen: false,
    items: [
      {
        id: "palette-task",
        kind: "effect",
        label: "任务编排",
        icon: "listTodo",
        accent: "#8B5CF6",
      },
    ],
  },
];

const EDGE_STYLE = {
  stroke: "#CBD5E1",
  strokeWidth: 2,
};

const INITIAL_WORKFLOW_EDGES: Edge[] = [
  {
    id: "start->webhook",
    source: "start",
    target: "webhook_trigger",
    sourceHandle: "out",
    targetHandle: "in",
    style: EDGE_STYLE,
  },
  {
    id: "webhook->fetch",
    source: "webhook_trigger",
    target: "fetch_order",
    sourceHandle: "out",
    targetHandle: "in",
    style: EDGE_STYLE,
  },
  {
    id: "fetch->switch",
    source: "fetch_order",
    target: "switch_biz_type",
    sourceHandle: "out",
    targetHandle: "in",
    style: EDGE_STYLE,
  },
  {
    id: "switch->assign",
    source: "switch_biz_type",
    target: "assign_task",
    sourceHandle: "branch-a",
    targetHandle: "in",
    style: EDGE_STYLE,
  },
  {
    id: "switch->wait",
    source: "switch_biz_type",
    target: "wait_callback",
    sourceHandle: "branch-b",
    targetHandle: "in",
    style: EDGE_STYLE,
  },
  {
    id: "assign->end-left",
    source: "assign_task",
    target: "end_left",
    sourceHandle: "out",
    targetHandle: "in",
    style: EDGE_STYLE,
  },
  {
    id: "wait->end-right",
    source: "wait_callback",
    target: "end_right",
    sourceHandle: "out",
    targetHandle: "in",
    style: EDGE_STYLE,
  },
];

const INITIAL_WORKFLOW_PANELS: Record<string, WorkflowNodePanel> = {
  assign_task: {
    tabs: ["base", "mapping", "retry"],
    fieldsByTab: {
      base: [
        {
          key: "command",
          label: "Shell 命令",
          type: "input",
          value: "printf '%s' \"$WORKFLOW_PARAMS\"",
        },
        {
          key: "shell",
          label: "解释器",
          type: "input",
          value: "sh",
        },
        {
          key: "workingDirectory",
          label: "工作目录",
          type: "input",
          value: "",
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "执行 Shell 脚本",
        },
        {
          key: "timeout",
          label: "超时时间 (ms)",
          type: "input",
          value: "5000",
        },
        {
          key: "nodeId",
          label: "节点 ID",
          type: "readonly",
          value: "assign_task",
        },
        {
          key: "note",
          label: "备注",
          type: "textarea",
          value:
            "通过本机 shell 执行命令；inputMapping 会序列化为 JSON 写入 stdin，同时注入 WORKFLOW_PARAMS 环境变量。",
        },
      ],
      mapping: [
        {
          key: "payload",
          label: "标准输入 / 参数",
          type: "textarea",
          value: "{\n  orderId: payload.orderId,\n  lane: payload.laneCode\n}",
        },
      ],
      retry: [
        {
          key: "retryPolicy",
          label: "失败重试",
          type: "select",
          value: "exponential_backoff",
        },
        {
          key: "maxAttempts",
          label: "最大重试次数",
          type: "input",
          value: "3",
        },
      ],
    },
  },
  effect_node: {
    tabs: ["base", "mapping", "retry"],
    fieldsByTab: {
      base: [
        {
          key: "command",
          label: "命令名称",
          type: "input",
          value: "",
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "新建副作用节点",
        },
        {
          key: "timeout",
          label: "超时时间 (ms)",
          type: "input",
          value: "5000",
        },
        {
          key: "nodeId",
          label: "节点 ID",
          type: "readonly",
          value: "effect_node",
        },
        {
          key: "note",
          label: "备注",
          type: "textarea",
          value: "",
        },
      ],
      mapping: [
        {
          key: "payload",
          label: "载荷",
          type: "textarea",
          value: "{\n  orderId: payload.orderId\n}",
        },
      ],
      retry: [
        {
          key: "retryPolicy",
          label: "失败重试",
          type: "select",
          value: "exponential_backoff",
        },
        {
          key: "maxAttempts",
          label: "最大重试次数",
          type: "input",
          value: "3",
        },
      ],
    },
  },
  end_left: {
    tabs: ["base"],
    fieldsByTab: {
      base: [
        {
          key: "result",
          label: "结束状态",
          type: "readonly",
          value: "success",
        },
        {
          key: "nodeId",
          label: "节点 ID",
          type: "readonly",
          value: "end_left",
        },
      ],
    },
  },
  end_right: {
    tabs: ["base"],
    fieldsByTab: {
      base: [
        {
          key: "result",
          label: "结束状态",
          type: "readonly",
          value: "waiting_callback",
        },
        {
          key: "nodeId",
          label: "节点 ID",
          type: "readonly",
          value: "end_right",
        },
      ],
    },
  },
  fetch_order: {
    tabs: ["base", "mapping", "retry", "error"],
    fieldsByTab: {
      base: [
        { key: "method", label: "请求方式", type: "select", value: "GET" },
        {
          key: "url",
          label: "请求 URL",
          type: "input",
          value: "https://jsonplaceholder.typicode.com/todos",
        },
        {
          key: "headers",
          label: "请求头",
          type: "textarea",
          value: "{\n  x-source: workflow-editor\n}",
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "查询订单",
        },
        {
          key: "timeout",
          label: "超时时间 (ms)",
          type: "input",
          value: "3000",
        },
        {
          key: "nodeId",
          label: "节点 ID",
          type: "readonly",
          value: "fetch_order",
        },
        {
          key: "onError",
          label: "错误处理策略",
          type: "select",
          value: "retry",
        },
        {
          key: "note",
          label: "备注",
          type: "textarea",
          value:
            "使用 HTTP GET / POST 请求外部接口，返回值会暴露在 input.data 中供后续节点使用。",
        },
      ],
      mapping: [
        {
          key: "inputFrom",
          label: "请求参数 / 请求体",
          type: "textarea",
          value: "{\n  userId: trigger.body.userId\n}",
        },
        {
          key: "outputTo",
          label: "出参保存",
          type: "textarea",
          value: "{\n  todos: response.data\n}",
        },
      ],
      retry: [
        {
          key: "retryPolicy",
          label: "重试策略",
          type: "select",
          value: "linear",
        },
        { key: "retryCount", label: "重试次数", type: "input", value: "2" },
      ],
      error: [
        {
          key: "fallback",
          label: "失败转向",
          type: "readonly",
          value: "notify_failure",
        },
        { key: "alarm", label: "告警级别", type: "select", value: "warning" },
      ],
    },
  },
  start: {
    tabs: ["base"],
    fieldsByTab: {
      base: [
        {
          key: "nodeName",
          label: "节点名称",
          type: "readonly",
          value: "Start",
        },
        { key: "entry", label: "入口模式", type: "readonly", value: "单入口" },
      ],
    },
  },
  switch_biz_type: {
    tabs: ["base", "mapping", "error"],
    fieldsByTab: {
      base: [
        {
          key: "expression",
          label: "分流表达式",
          type: "input",
          value: "payload.bizType",
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "业务分流",
        },
        {
          key: "nodeId",
          label: "节点 ID",
          type: "readonly",
          value: "switch_biz_type",
        },
        {
          key: "fallback",
          label: "默认分支",
          type: "select",
          value: "bizType=B",
        },
      ],
      mapping: [
        {
          key: "caseA",
          label: "分支 A 条件",
          type: "readonly",
          value: "bizType === 'A'",
        },
        {
          key: "caseB",
          label: "分支 B 条件",
          type: "readonly",
          value: "bizType === 'B'",
        },
      ],
      error: [
        {
          key: "unknown",
          label: "未知分支策略",
          type: "select",
          value: "route_to_manual_review",
        },
      ],
    },
  },
  wait_callback: {
    tabs: ["base", "retry", "error"],
    fieldsByTab: {
      base: [
        {
          key: "waitEvent",
          label: "等待事件",
          type: "input",
          value: "device.sorting.callback",
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "等待设备回调",
        },
        {
          key: "timeout",
          label: "最长等待 (ms)",
          type: "input",
          value: "15000",
        },
        {
          key: "nodeId",
          label: "节点 ID",
          type: "readonly",
          value: "wait_callback",
        },
      ],
      retry: [
        { key: "polling", label: "轮询补偿", type: "select", value: "enabled" },
        {
          key: "interval",
          label: "补偿间隔 (ms)",
          type: "input",
          value: "2000",
        },
      ],
      error: [
        {
          key: "timeoutAction",
          label: "超时处理",
          type: "select",
          value: "mark_pending",
        },
      ],
    },
  },
  webhook_trigger: {
    tabs: ["base", "mapping", "error"],
    fieldsByTab: {
      base: [
        {
          key: "path",
          label: "Webhook Path",
          type: "input",
          value: "/api/workflow/inbound-order",
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "接收入库订单",
        },
        { key: "method", label: "请求方式", type: "select", value: "POST" },
        {
          key: "nodeId",
          label: "节点 ID",
          type: "readonly",
          value: "webhook_trigger",
        },
      ],
      mapping: [
        {
          key: "payload",
          label: "原始载荷",
          type: "textarea",
          value: "{\n  orderId: body.orderId,\n  laneCode: body.laneCode\n}",
        },
      ],
      error: [
        {
          key: "onInvalid",
          label: "签名失败处理",
          type: "select",
          value: "reject_401",
        },
      ],
    },
  },
};

const slugifyNodeId = (value: string) =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");

const getPaletteBaseNodeId = (item: WorkflowPaletteItem) => {
  const labelId = slugifyNodeId(item.label);

  if (labelId) {
    return labelId;
  }

  const paletteId = slugifyNodeId(item.id.replace(/^palette-/, ""));

  if (paletteId) {
    return paletteId;
  }

  const kindId = slugifyNodeId(item.kind);

  return kindId || "node";
};

const clonePanel = (nodeId: keyof typeof INITIAL_WORKFLOW_PANELS) =>
  structuredClone(INITIAL_WORKFLOW_PANELS[nodeId]) as WorkflowNodePanel;

const setFieldValue = (
  panel: WorkflowNodePanel,
  fieldKey: string,
  value: string,
) => {
  panel.tabs.forEach((tab) => {
    panel.fieldsByTab[tab]?.forEach((field) => {
      if (field.key === fieldKey) {
        field.value = value;
      }
    });
  });
};

const getUniqueNodeId = (baseId: string, existingNodes: WorkflowFlowNode[]) => {
  const existingIds = new Set(existingNodes.map((node) => node.id));

  if (!existingIds.has(baseId)) {
    return baseId;
  }

  let counter = 2;

  while (existingIds.has(`${baseId}_${counter}`)) {
    counter += 1;
  }

  return `${baseId}_${counter}`;
};

export const createWorkflowNodeDraft = (
  item: WorkflowPaletteItem,
  position: WorkflowFlowNode["position"],
  existingNodes: WorkflowFlowNode[],
): WorkflowNodeDraft => {
  const baseNodeId = getPaletteBaseNodeId(item);

  switch (item.id) {
    case "palette-start": {
      const nodeId = getUniqueNodeId("start", existingNodes);
      const panel = clonePanel("start");

      setFieldValue(panel, "nodeName", "Start");

      return {
        node: {
          id: nodeId,
          type: "terminal",
          position,
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: {
            accent: "#10B981",
            icon: "play",
            kind: "start",
            nodeKey: nodeId,
            title: "Start",
          },
        },
        panel,
      };
    }
    case "palette-end": {
      const nodeId = getUniqueNodeId("end", existingNodes);
      const panel = clonePanel("end_left");

      setFieldValue(panel, "nodeId", nodeId);
      setFieldValue(panel, "nodeName", "End");

      return {
        node: {
          id: nodeId,
          type: "terminal",
          position,
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: {
            accent: "#EF4444",
            icon: "shield",
            kind: "end",
            nodeKey: nodeId,
            title: "End",
          },
        },
        panel,
      };
    }
    case "palette-webhook": {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("webhook_trigger");
      const subtitle = "新建 Webhook Trigger";

      setFieldValue(panel, "nodeId", nodeId);
      setFieldValue(panel, "nodeName", subtitle);

      return {
        node: {
          id: nodeId,
          type: "workflow-card",
          position,
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: {
            accent: "#6366F1",
            icon: "webhook",
            kind: "trigger",
            nodeKey: nodeId,
            subtitle,
            title: "Webhook Trigger",
          },
        },
        panel,
      };
    }
    case "palette-fetch": {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("fetch_order");
      const subtitle = "新建查询节点";

      setFieldValue(panel, "nodeId", nodeId);
      setFieldValue(panel, "nodeName", subtitle);

      return {
        node: {
          id: nodeId,
          type: "workflow-card",
          position,
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: {
            accent: "#3B82F6",
            icon: "database",
            kind: "fetch",
            nodeKey: nodeId,
            subtitle,
            title: "Fetch",
          },
        },
        panel,
      };
    }
    case "palette-if-else": {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("switch_biz_type");
      const subtitle = "新建条件分支";

      setFieldValue(panel, "nodeId", nodeId);
      setFieldValue(panel, "nodeName", subtitle);
      setFieldValue(panel, "expression", "payload.condition === true");
      setFieldValue(panel, "fallback", "else");

      return {
        node: {
          id: nodeId,
          type: "workflow-card",
          position,
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: {
            accent: "#F97316",
            icon: "gitBranch",
            kind: "if-else",
            nodeKey: nodeId,
            subtitle,
            title: "If / Else",
          },
        },
        panel,
      };
    }
    case "palette-switch": {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("switch_biz_type");
      const subtitle = "新建业务分流";

      setFieldValue(panel, "nodeId", nodeId);
      setFieldValue(panel, "nodeName", subtitle);

      return {
        node: {
          id: nodeId,
          type: "workflow-card",
          position,
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: {
            accent: "#EC4899",
            icon: "gitBranch",
            kind: "switch",
            nodeKey: nodeId,
            subtitle,
            title: "Switch",
          },
        },
        panel,
      };
    }
    case "palette-wait": {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("wait_callback");
      const subtitle = "新建等待节点";

      setFieldValue(panel, "nodeId", nodeId);
      setFieldValue(panel, "nodeName", subtitle);

      return {
        node: {
          id: nodeId,
          type: "workflow-card",
          position,
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: {
            accent: "#F59E0B",
            icon: "clock",
            kind: "wait",
            nodeKey: nodeId,
            subtitle,
            title: "Wait",
          },
        },
        panel,
      };
    }
    default: {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("assign_task");
      const subtitle = `新建${item.label}`;
      const titleMap: Partial<Record<WorkflowPaletteItem["id"], string>> = {
        "palette-shell": "Shell",
        "palette-respond": "Respond",
        "palette-subflow": "Sub-Workflow",
        "palette-task": "Task",
      };
      const title = titleMap[item.id] ?? item.label;

      if (item.id !== "palette-shell") {
        const genericPanel = clonePanel("effect_node");
        setFieldValue(genericPanel, "nodeId", nodeId);
        setFieldValue(genericPanel, "nodeName", subtitle);

        return {
          node: {
            id: nodeId,
            type: "workflow-card",
            position,
            sourcePosition: Position.Right,
            targetPosition: Position.Left,
            data: {
              accent: item.accent,
              icon: item.icon,
              kind: item.kind,
              nodeKey: nodeId,
              subtitle,
              title,
            },
          },
          panel: genericPanel,
        };
      }

      setFieldValue(panel, "nodeId", nodeId);
      setFieldValue(panel, "nodeName", subtitle);

      return {
        node: {
          id: nodeId,
          type: "workflow-card",
          position,
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: {
            accent: item.accent,
            icon: item.icon,
            kind: item.kind,
            nodeKey: nodeId,
            subtitle,
            title,
          },
        },
        panel,
      };
    }
  }
};

export const createWorkflowEdges = () =>
  structuredClone(INITIAL_WORKFLOW_EDGES) as Edge[];
export const createWorkflowPanels = () =>
  structuredClone(INITIAL_WORKFLOW_PANELS) as Record<string, WorkflowNodePanel>;
