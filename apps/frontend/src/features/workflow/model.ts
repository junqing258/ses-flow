import type { Component } from "vue";
import {
  Position,
  type Edge,
  type Node,
  type XYPosition,
} from "@vue-flow/core";
import * as LucideIcons from "lucide-vue-next";
import {
  Activity,
  Code2,
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
  code: Code2,
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
export type WorkflowIconValue = WorkflowIconKey | string;
export type WorkflowNodeKind =
  | "start"
  | "trigger"
  | "sub-workflow"
  | "fetch"
  | "db-query"
  | "set-state"
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

export interface WorkflowFieldOption {
  label: string;
  value: string;
}

export interface WorkflowReferenceSummary {
  workflowId: string;
  workflowKey: string;
}

export interface WorkflowNodeData {
  active?: boolean;
  accent: string;
  branchHandles?: WorkflowBranchHandle[];
  executionStatus?: WorkflowExecutionStatus;
  icon: WorkflowIconValue;
  kind: WorkflowNodeKind;
  nodeKey: string;
  runnerType?: string;
  status?: "draft" | "published";
  subtitle?: string;
  title: string;
}

export interface WorkflowField {
  key: string;
  label: string;
  options?: WorkflowFieldOption[];
  type: WorkflowFieldType;
  value: string;
}

export interface WorkflowBranchHandle {
  id: string;
  isDefault?: boolean;
  label: string;
}

export interface WorkflowNodePanel {
  fieldsByTab: Partial<Record<WorkflowTabId, WorkflowField[]>>;
  tabs: WorkflowTabId[];
}

export interface WorkflowPaletteItem {
  accent: string;
  icon: WorkflowIconValue;
  id: string;
  kind: WorkflowNodeKind;
  label: string;
  nodeDescriptor?: WorkflowNodeDescriptor;
  runnerType?: string;
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

export type WorkflowNodePosition = XYPosition;
export interface WorkflowExistingNode {
  id: string;
}
export type CreateWorkflowNodeDraft = (
  item: WorkflowPaletteItem,
  position: WorkflowNodePosition,
  existingNodes: readonly WorkflowExistingNode[],
) => WorkflowNodeDraft;

export interface WorkflowJsonSchemaProperty {
  default?: unknown;
  enum?: unknown[];
  title?: string;
  type?: string;
  ["x-component"]?: string;
  ["x-options"]?: unknown[];
  ["x-tab"]?: string;
}

export interface WorkflowNodeDescriptor {
  category: string;
  configSchema?: {
    properties?: Record<string, WorkflowJsonSchemaProperty>;
    required?: string[];
    type?: string;
  };
  defaults?: Record<string, unknown> | null;
  description?: string;
  displayName: string;
  endpoint?: string | null;
  color?: string | null;
  icon?: string | null;
  id: string;
  kind: WorkflowNodeKind | string;
  pluginAppId?: string | null;
  pluginAppName?: string | null;
  runnerType: string;
  status: "stable" | "beta" | "deprecated";
  timeoutMs?: number;
  transport?: "builtin" | "http" | "grpc" | "process";
  version: string;
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

export const WORKFLOW_EDGE_TYPE = "default";
export const WORKFLOW_EDGE_STYLE = {
  stroke: "#CBD5E1",
  strokeWidth: 2,
};

const SWITCH_BRANCH_FIELD_KEY_PATTERN = /^branch:([^:]+):label$/;
const DEFAULT_SWITCH_BRANCH_LABELS = ["A", "B"];
const DEFAULT_SELECT_FIELD_OPTIONS: Partial<
  Record<string, WorkflowFieldOption[]>
> = {
  alarm: [
    { label: "warning", value: "warning" },
    { label: "critical", value: "critical" },
  ],
  method: [
    { label: "GET", value: "GET" },
    { label: "POST", value: "POST" },
  ],
  mode: [
    { label: "read", value: "read" },
    { label: "write", value: "write" },
  ],
  onError: [
    { label: "retry", value: "retry" },
    { label: "fail_fast", value: "fail_fast" },
  ],
  onInvalid: [
    { label: "reject_401", value: "reject_401" },
    { label: "ignore", value: "ignore" },
  ],
  responseMode: [
    { label: "async_ack", value: "async_ack" },
    { label: "sync", value: "sync" },
  ],
  polling: [
    { label: "enabled", value: "enabled" },
    { label: "disabled", value: "disabled" },
  ],
  retryPolicy: [
    { label: "linear", value: "linear" },
    { label: "exponential_backoff", value: "exponential_backoff" },
    { label: "none", value: "none" },
  ],
  timeoutAction: [
    { label: "mark_pending", value: "mark_pending" },
    { label: "fail_workflow", value: "fail_workflow" },
  ],
  unknown: [
    {
      label: "route_to_manual_review",
      value: "route_to_manual_review",
    },
    { label: "use_default_branch", value: "use_default_branch" },
  ],
};

const isWorkflowNodeKind = (value: string): value is WorkflowNodeKind =>
  [
    "start",
    "trigger",
    "sub-workflow",
    "fetch",
    "db-query",
    "set-state",
    "if-else",
    "switch",
    "shell",
    "effect",
    "wait",
    "end",
    "branch-label",
  ].includes(value);

const WORKFLOW_ICON_HTTP_URL_PATTERN = /^https?:\/\//i;
const LUCIDE_ICON_ALIASES: Record<string, WorkflowIconKey> = {
  http: "webhook",
};
const LUCIDE_ICON_LIBRARY = LucideIcons as unknown as Record<string, Component>;

export type ResolvedWorkflowIcon =
  | { kind: "component"; component: Component }
  | { kind: "image"; src: string };

const normalizeWorkflowIconName = (value: string) =>
  value
    .trim()
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[^a-zA-Z0-9]+/g, " ")
    .split(/\s+/)
    .filter(Boolean)
    .map((token) => token.charAt(0).toUpperCase() + token.slice(1))
    .join("");

const resolveWorkflowIconComponent = (
  value: string | null | undefined,
): Component => {
  const normalizedValue = value?.trim();

  if (!normalizedValue) {
    return WORKFLOW_ICON_MAP.activity;
  }

  if (normalizedValue in WORKFLOW_ICON_MAP) {
    return WORKFLOW_ICON_MAP[normalizedValue as WorkflowIconKey];
  }

  const alias =
    LUCIDE_ICON_ALIASES[
      normalizedValue.toLowerCase().replace(/[^a-z0-9]+/g, "")
    ];
  if (alias) {
    return WORKFLOW_ICON_MAP[alias];
  }

  const lucideComponent = LUCIDE_ICON_LIBRARY[
    normalizeWorkflowIconName(normalizedValue)
  ];
  return lucideComponent ?? WORKFLOW_ICON_MAP.activity;
};

const normalizeWorkflowIconValue = (
  value: string | null | undefined,
): WorkflowIconValue => value?.trim() || "activity";

export const resolveWorkflowIcon = (
  value: string | null | undefined,
): ResolvedWorkflowIcon => {
  const normalizedValue = value?.trim();

  if (
    normalizedValue &&
    WORKFLOW_ICON_HTTP_URL_PATTERN.test(normalizedValue)
  ) {
    return {
      kind: "image",
      src: normalizedValue,
    };
  }

  return {
    component: resolveWorkflowIconComponent(normalizedValue),
    kind: "component",
  };
};

const toWorkflowFieldType = (
  property: WorkflowJsonSchemaProperty,
): WorkflowFieldType => {
  const component = property["x-component"];

  if (
    component === "select" ||
    component === "radio" ||
    Array.isArray(property.enum) ||
    Array.isArray(property["x-options"])
  ) {
    return "select";
  }

  if (component === "textarea" || property.type === "object") {
    return "textarea";
  }

  return "input";
};

const toFieldOptions = (
  property: WorkflowJsonSchemaProperty,
): WorkflowFieldOption[] | undefined => {
  const options = Array.isArray(property["x-options"])
    ? property["x-options"]
    : property.enum;

  if (!Array.isArray(options)) {
    return undefined;
  }

  return options.map((option) => ({
    label: String(option),
    value: String(option),
  }));
};

const stringifyFieldValue = (value: unknown): string => {
  if (value === undefined || value === null) {
    return "";
  }

  if (typeof value === "string") {
    return value;
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }

  return JSON.stringify(value, null, 2);
};

const createDescriptorConfigFields = (
  descriptor: WorkflowNodeDescriptor,
): WorkflowField[] =>
  Object.entries(descriptor.configSchema?.properties ?? {}).map(
    ([key, property]) => ({
      key: `config:${key}`,
      label: property.title ?? key,
      options: toFieldOptions(property),
      type: toWorkflowFieldType(property),
      value: stringifyFieldValue(
        descriptor.defaults?.[key] ?? property.default ?? "",
      ),
    }),
  );

const createPluginNodePanel = (
  descriptor: WorkflowNodeDescriptor,
  nodeId: string,
  subtitle: string,
): WorkflowNodePanel => ({
  tabs: ["base", "mapping", "retry"],
  fieldsByTab: {
    base: [
      ...createDescriptorConfigFields(descriptor),
      {
        key: "nodeName",
        label: "节点名称",
        type: "input",
        value: subtitle,
      },
      {
        key: "timeout",
        label: "超时时间 (ms)",
        type: "input",
        value: descriptor.timeoutMs ? String(descriptor.timeoutMs) : "5000",
      },
      {
        key: "runnerType",
        label: "Runner 类型",
        type: "readonly",
        value: descriptor.runnerType,
      },
      {
        key: "nodeId",
        label: "节点 ID",
        type: "readonly",
        value: nodeId,
      },
      {
        key: "note",
        label: "备注",
        type: "textarea",
        value: descriptor.description ?? "",
      },
    ],
    mapping: [
      {
        key: "payload",
        label: "插件输入",
        type: "textarea",
        value: "{{input}}",
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
});

export const createSwitchBranchHandleId = (index: number) => {
  if (index < 26) {
    return `branch-${String.fromCharCode(97 + index)}`;
  }

  return `branch-${index + 1}`;
};

const createSwitchBranchField = (
  branchId: string,
  label: string,
): WorkflowField => ({
  key: `branch:${branchId}:label`,
  label: `分支 ${label || branchId}`,
  type: "input",
  value: label,
});

export const createDefaultSwitchBranches = (): WorkflowBranchHandle[] =>
  DEFAULT_SWITCH_BRANCH_LABELS.map((label, index) => ({
    id: createSwitchBranchHandleId(index),
    label,
  }));

const isSwitchBranchFieldKey = (fieldKey: string) =>
  SWITCH_BRANCH_FIELD_KEY_PATTERN.test(fieldKey);

export const getSwitchBranches = (
  panel: WorkflowNodePanel | undefined,
): WorkflowBranchHandle[] => {
  const mappingFields = panel?.fieldsByTab.mapping ?? [];
  const dynamicBranches = mappingFields.flatMap<WorkflowBranchHandle>(
    (field) => {
      const match = field.key.match(SWITCH_BRANCH_FIELD_KEY_PATTERN);

      if (!match?.[1]) {
        return [];
      }

      return [
        {
          id: match[1],
          label: field.value.trim() || field.label || match[1],
        },
      ];
    },
  );

  if (dynamicBranches.length > 0) {
    return dynamicBranches;
  }

  return createDefaultSwitchBranches();
};

export const setSwitchBranches = (
  panel: WorkflowNodePanel,
  branches: WorkflowBranchHandle[],
) => {
  const mappingFields = panel.fieldsByTab.mapping ?? [];
  const preservedFields = mappingFields.filter(
    (field) => !isSwitchBranchFieldKey(field.key),
  );

  panel.fieldsByTab.mapping = [
    ...preservedFields,
    ...branches.map((branch) =>
      createSwitchBranchField(branch.id, branch.label),
    ),
  ];
};

export const getSwitchFallbackHandle = (
  panel: WorkflowNodePanel | undefined,
): string | undefined => {
  const fallbackValue =
    panel?.fieldsByTab.base?.find((field) => field.key === "fallback")?.value ??
    "";
  const trimmed = fallbackValue.trim();
  const branches = getSwitchBranches(panel);

  if (!trimmed) {
    return undefined;
  }

  if (branches.some((branch) => branch.id === trimmed)) {
    return trimmed;
  }

  return undefined;
};

const withCurrentFieldValue = (
  options: WorkflowFieldOption[],
  currentValue: string,
) => {
  const trimmed = currentValue.trim();

  if (!trimmed || options.some((option) => option.value === trimmed)) {
    return options;
  }

  return [...options, { label: trimmed, value: trimmed }];
};

export const getWorkflowFieldSelectOptions = (
  panel: WorkflowNodePanel | undefined,
  field: WorkflowField,
  overrideOptions?: WorkflowFieldOption[],
): WorkflowFieldOption[] => {
  if (field.type !== "select") {
    return [];
  }

  if (overrideOptions && overrideOptions.length > 0) {
    return withCurrentFieldValue(overrideOptions, field.value);
  }

  if (field.options && field.options.length > 0) {
    return withCurrentFieldValue(field.options, field.value);
  }

  if (field.key === "fallback") {
    return withCurrentFieldValue(
      getSwitchBranches(panel).map((branch) => ({
        label: branch.label || branch.id,
        value: branch.id,
      })),
      field.value,
    );
  }

  return withCurrentFieldValue(
    DEFAULT_SELECT_FIELD_OPTIONS[field.key] ?? [],
    field.value,
  );
};

export const resolveWorkflowReferenceId = (
  workflowRef: string,
  workflows: WorkflowReferenceSummary[],
): string | undefined => {
  const normalizedRef = workflowRef.trim();

  if (!normalizedRef) {
    return undefined;
  }

  const exactIdMatch = workflows.find(
    (workflow) => workflow.workflowId.trim() === normalizedRef,
  );

  if (exactIdMatch) {
    return exactIdMatch.workflowId;
  }

  return workflows.find(
    (workflow) => workflow.workflowKey.trim() === normalizedRef,
  )?.workflowId;
};

export const setSwitchFallbackHandle = (
  panel: WorkflowNodePanel,
  branchId: string,
) => {
  panel.fieldsByTab.base?.forEach((field) => {
    if (field.key === "fallback") {
      field.value = branchId;
    }
  });
};

export const getBranchHandlesForNode = (
  kind: WorkflowNodeKind,
  panel: WorkflowNodePanel | undefined,
): WorkflowBranchHandle[] | undefined => {
  if (kind === "switch") {
    const fallbackHandle = getSwitchFallbackHandle(panel);

    return getSwitchBranches(panel).map((branch) => ({
      ...branch,
      isDefault: branch.id === fallbackHandle,
    }));
  }

  if (kind === "if-else") {
    return [
      { id: "branch-a", label: "then" },
      { id: "branch-b", isDefault: true, label: "else" },
    ];
  }

  return undefined;
};

export const syncBranchHandlesForNode = (
  node: WorkflowFlowNode,
  panel: WorkflowNodePanel | undefined,
): WorkflowFlowNode => {
  const branchHandles = getBranchHandlesForNode(node.data.kind, panel);

  if (!branchHandles) {
    return node;
  }

  return {
    ...node,
    data: {
      ...node.data,
      branchHandles,
    },
  };
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
      {
        id: "palette-if-else",
        kind: "if-else",
        label: "If / Else",
        icon: "gitBranch",
        accent: "#F97316",
      },
      {
        id: "palette-switch",
        kind: "switch",
        label: "Switch",
        icon: "gitBranch",
        accent: "#EC4899",
      },
      {
        id: "palette-subflow",
        kind: "sub-workflow",
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
      {
        id: "palette-db-query",
        kind: "db-query",
        label: "DB Query",
        icon: "database",
        accent: "#2563EB",
      },
      {
        id: "palette-set-state",
        kind: "set-state",
        label: "Set State",
        icon: "database",
        accent: "#14B8A6",
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
      {
        id: "palette-code",
        kind: "effect",
        label: "Code",
        icon: "code",
        accent: "#0F766E",
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
];

export const LEGACY_TASK_PALETTE_ITEM: WorkflowPaletteItem = {
  id: "palette-task",
  kind: "effect",
  label: "Task",
  icon: "listTodo",
  accent: "#8B5CF6",
};

const getDynamicPaletteAccent = (descriptor: WorkflowNodeDescriptor) => {
  const configuredColor = descriptor.color?.trim();
  if (configuredColor) {
    return configuredColor;
  }

  if (descriptor.transport === "http") {
    return "#0EA5E9";
  }

  if (descriptor.transport === "grpc") {
    return "#14B8A6";
  }

  return "#8B5CF6";
};

const toPaletteCategoryId = (label: string) =>
  `dynamic-${
    label
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "plugins"
  }`;

const prettifyPluginAppLabel = (value: string) => {
  const tokens = value
    .trim()
    .replace(/[_-]+/g, " ")
    .split(/\s+/)
    .filter(Boolean);

  const normalizedTokens =
    tokens.length > 1 &&
    ["plugin", "plugins", "bridge", "app"].includes(
      tokens[tokens.length - 1]?.toLowerCase() ?? "",
    )
      ? tokens.slice(0, -1)
      : tokens;

  return normalizedTokens
    .map((token) => {
      if (/^[a-z]{2,4}$/i.test(token)) {
        return token.toUpperCase();
      }

      return token.charAt(0).toUpperCase() + token.slice(1).toLowerCase();
    })
    .join(" ");
};

const getPluginEndpointGroupId = (descriptor: WorkflowNodeDescriptor) => {
  if (!descriptor.endpoint) {
    return null;
  }

  try {
    const endpointUrl = new URL(descriptor.endpoint);
    return `${endpointUrl.host}${endpointUrl.pathname.replace(/\/+$/, "")}`;
  } catch {
    return descriptor.endpoint;
  }
};

const getPluginEndpointGroupLabel = (descriptor: WorkflowNodeDescriptor) => {
  if (!descriptor.endpoint) {
    return null;
  }

  try {
    const endpointUrl = new URL(descriptor.endpoint);
    return endpointUrl.host;
  } catch {
    return descriptor.endpoint;
  }
};

const getDynamicPaletteGroup = (descriptor: WorkflowNodeDescriptor) => {
  if (!descriptor.runnerType.startsWith("plugin:")) {
    const categoryLabel = descriptor.category.trim() || "动态节点";

    return {
      id: toPaletteCategoryId(categoryLabel),
      label: categoryLabel,
    };
  }

  const appId = descriptor.pluginAppId?.trim();
  const appName = descriptor.pluginAppName?.trim();
  const endpointGroupId = getPluginEndpointGroupId(descriptor);
  const endpointGroupLabel = getPluginEndpointGroupLabel(descriptor);

  const groupIdSource =
    appId || endpointGroupId || descriptor.category || descriptor.id;
  const groupLabelSource =
    appName ||
    (appId ? prettifyPluginAppLabel(appId) : null) ||
    endpointGroupLabel ||
    descriptor.category.trim() ||
    descriptor.displayName;

  return {
    id: `plugin-app-${
      groupIdSource
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "") || "plugins"
    }`,
    label: groupLabelSource,
  };
};

const toPaletteItemKind = (
  descriptor: WorkflowNodeDescriptor,
): WorkflowNodeKind => {
  if (
    typeof descriptor.kind === "string" &&
    isWorkflowNodeKind(descriptor.kind)
  ) {
    return descriptor.kind;
  }

  if (descriptor.runnerType.startsWith("plugin:")) {
    return "effect";
  }

  return "effect";
};

const createDynamicPaletteItem = (
  descriptor: WorkflowNodeDescriptor,
): WorkflowPaletteItem => ({
  accent: getDynamicPaletteAccent(descriptor),
  icon: normalizeWorkflowIconValue(descriptor.icon),
  id: `palette-${descriptor.id.replace(/_/g, "-")}`,
  kind: toPaletteItemKind(descriptor),
  label: descriptor.displayName,
  nodeDescriptor: descriptor,
  runnerType: descriptor.runnerType,
});

export const createWorkflowPaletteCategories = (
  descriptors: WorkflowNodeDescriptor[] = [],
): WorkflowPaletteCategory[] => {
  const categories = structuredClone(
    WORKFLOW_PALETTE_CATEGORIES,
  ) as WorkflowPaletteCategory[];
  const dynamicCategoryMap = new Map<string, WorkflowPaletteCategory>();
  const dynamicDescriptors = descriptors
    .filter((descriptor) => descriptor.status !== "deprecated")
    .sort((left, right) => left.displayName.localeCompare(right.displayName));

  dynamicDescriptors.forEach((descriptor) => {
    const group = getDynamicPaletteGroup(descriptor);
    const item = createDynamicPaletteItem(descriptor);
    const existingCategory = dynamicCategoryMap.get(group.id);

    if (existingCategory) {
      existingCategory.items.push(item);
      return;
    }

    dynamicCategoryMap.set(group.id, {
      defaultOpen: false,
      icon: "activity",
      id: group.id,
      items: [item],
      label: group.label,
    });
  });

  categories.push(
    ...Array.from(dynamicCategoryMap.values()).sort((left, right) =>
      left.label.localeCompare(right.label),
    ),
  );

  return categories;
};

const INITIAL_WORKFLOW_EDGES: Edge[] = [
  {
    id: "start->webhook",
    source: "start",
    target: "webhook_trigger",
    sourceHandle: "out",
    targetHandle: "in",
    type: WORKFLOW_EDGE_TYPE,
    style: WORKFLOW_EDGE_STYLE,
  },
  {
    id: "webhook->fetch",
    source: "webhook_trigger",
    target: "fetch_order",
    sourceHandle: "out",
    targetHandle: "in",
    type: WORKFLOW_EDGE_TYPE,
    style: WORKFLOW_EDGE_STYLE,
  },
  {
    id: "fetch->switch",
    source: "fetch_order",
    target: "switch_biz_type",
    sourceHandle: "out",
    targetHandle: "in",
    type: WORKFLOW_EDGE_TYPE,
    style: WORKFLOW_EDGE_STYLE,
  },
  {
    id: "switch->assign",
    source: "switch_biz_type",
    target: "assign_task",
    sourceHandle: "branch-a",
    targetHandle: "in",
    type: WORKFLOW_EDGE_TYPE,
    style: WORKFLOW_EDGE_STYLE,
  },
  {
    id: "switch->wait",
    source: "switch_biz_type",
    target: "wait_callback",
    sourceHandle: "branch-b",
    targetHandle: "in",
    type: WORKFLOW_EDGE_TYPE,
    style: WORKFLOW_EDGE_STYLE,
  },
  {
    id: "assign->end-left",
    source: "assign_task",
    target: "end_left",
    sourceHandle: "out",
    targetHandle: "in",
    type: WORKFLOW_EDGE_TYPE,
    style: WORKFLOW_EDGE_STYLE,
  },
  {
    id: "wait->end-right",
    source: "wait_callback",
    target: "end_right",
    sourceHandle: "out",
    targetHandle: "in",
    type: WORKFLOW_EDGE_TYPE,
    style: WORKFLOW_EDGE_STYLE,
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
  sub_workflow: {
    tabs: ["base", "mapping", "retry"],
    fieldsByTab: {
      base: [
        {
          key: "workflowRef",
          label: "子工作流",
          type: "select",
          value: "",
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "调用子工作流",
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
          value: "sub_workflow",
        },
        {
          key: "note",
          label: "备注",
          type: "textarea",
          value:
            "选择一个已注册 workflow 作为子流程执行，inputMapping 会作为子流程输入传入。",
        },
      ],
      mapping: [
        {
          key: "payload",
          label: "子流程输入",
          type: "textarea",
          value: "{{input}}",
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
  code_node: {
    tabs: ["base", "mapping", "retry"],
    fieldsByTab: {
      base: [
        {
          key: "source",
          label: "代码内容",
          type: "textarea",
          value:
            "return {\n  output: {\n    ok: true,\n    received: params,\n  },\n};",
        },
        {
          key: "language",
          label: "语言",
          type: "select",
          value: "javascript",
          options: [
            { label: "javascript", value: "javascript" },
            { label: "typescript", value: "typescript" },
          ],
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "执行代码逻辑",
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
          value: "code_node",
        },
        {
          key: "note",
          label: "备注",
          type: "textarea",
          value:
            "使用 Runner 的 Code 节点执行内联 JavaScript；可读取 trigger / input / state / env / params。",
        },
      ],
      mapping: [
        {
          key: "payload",
          label: "入参 / params",
          type: "textarea",
          value:
            "{\n  orderId: input.orderId,\n  requestId: trigger.headers.requestId\n}",
        },
      ],
      retry: [
        {
          key: "retryPolicy",
          label: "失败重试",
          type: "select",
          value: "none",
        },
        {
          key: "maxAttempts",
          label: "最大重试次数",
          type: "input",
          value: "1",
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
  db_query: {
    tabs: ["base", "mapping", "retry"],
    fieldsByTab: {
      base: [
        {
          key: "connectionKey",
          label: "连接键",
          type: "input",
          value: "default",
        },
        {
          key: "mode",
          label: "执行模式",
          type: "select",
          value: "read",
        },
        {
          key: "sql",
          label: "SQL",
          type: "textarea",
          value:
            "select *\nfrom orders\nwhere order_no = :order_no\nlimit 20",
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "查询数据库",
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
          value: "db_query",
        },
        {
          key: "note",
          label: "备注",
          type: "textarea",
          value:
            "使用服务端白名单环境变量 SES_FLOW_DB_<KEY>_URL 连接 PostgreSQL，SQL 支持 :name 命名参数。",
        },
      ],
      mapping: [
        {
          key: "params",
          label: "SQL 参数",
          type: "textarea",
          value: "{\n  order_no: trigger.body.orderNo\n}",
        },
      ],
      retry: [
        {
          key: "retryPolicy",
          label: "重试策略",
          type: "select",
          value: "none",
        },
        { key: "retryCount", label: "重试次数", type: "input", value: "1" },
      ],
    },
  },
  set_state: {
    tabs: ["base", "mapping"],
    fieldsByTab: {
      base: [
        {
          key: "statePath",
          label: "状态路径",
          type: "input",
          value: "statePatch",
        },
        {
          key: "nodeName",
          label: "节点名称",
          type: "input",
          value: "设置状态",
        },
        {
          key: "nodeId",
          label: "节点 ID",
          type: "readonly",
          value: "set_state",
        },
        {
          key: "note",
          label: "备注",
          type: "textarea",
          value:
            "将输入值写入 workflow state 的指定路径，后续节点可通过 state.* 继续引用。",
        },
      ],
      mapping: [
        {
          key: "value",
          label: "写入值",
          type: "textarea",
          value: "{\n  handledBy: input.route\n}",
        },
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
          value: "branch-b",
        },
      ],
      mapping: [
        createSwitchBranchField("branch-a", "A"),
        createSwitchBranchField("branch-b", "B"),
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
          key: "responseMode",
          label: "响应模式",
          type: "select",
          value: "async_ack",
        },
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

export const createWorkflowPaletteItemMap = (
  categories: WorkflowPaletteCategory[],
) =>
  categories
    .flatMap((category) => category.items)
    .reduce<Record<string, WorkflowPaletteItem>>((accumulator, item) => {
      accumulator[item.id] = item;
      return accumulator;
    }, {});

const createFallbackPluginPaletteItem = (
  runnerType: string,
): WorkflowPaletteItem => ({
  accent: "#8B5CF6",
  icon: "activity",
  id: `palette-${runnerType.replace(/[^a-z0-9]+/gi, "-").toLowerCase()}`,
  kind: "effect",
  label: runnerType.replace(/^plugin:/, "") || runnerType,
  runnerType,
});

export const resolvePaletteItemForRunnerType = (
  runnerType: string,
  categories: WorkflowPaletteCategory[] = WORKFLOW_PALETTE_CATEGORIES,
): WorkflowPaletteItem => {
  const items = categories.flatMap((category) => category.items);
  const matchedItem = items.find((item) => item.runnerType === runnerType);

  if (matchedItem) {
    return matchedItem;
  }

  if (runnerType.startsWith("plugin:")) {
    return createFallbackPluginPaletteItem(runnerType);
  }

  switch (runnerType) {
    case "start":
      return items.find((item) => item.id === "palette-start") ?? items[0];
    case "end":
      return items.find((item) => item.id === "palette-end") ?? items[0];
    case "fetch":
      return items.find((item) => item.id === "palette-fetch") ?? items[0];
    case "db_query":
      return items.find((item) => item.id === "palette-db-query") ?? items[0];
    case "set_state":
      return items.find((item) => item.id === "palette-set-state") ?? items[0];
    case "switch":
      return items.find((item) => item.id === "palette-switch") ?? items[0];
    case "if_else":
      return items.find((item) => item.id === "palette-if-else") ?? items[0];
    case "wait":
      return items.find((item) => item.id === "palette-wait") ?? items[0];
    case "task":
      return LEGACY_TASK_PALETTE_ITEM;
    case "respond":
      return items.find((item) => item.id === "palette-respond") ?? items[0];
    case "sub_workflow":
      return items.find((item) => item.id === "palette-subflow") ?? items[0];
    case "code":
      return items.find((item) => item.id === "palette-code") ?? items[0];
    case "shell":
      return items.find((item) => item.id === "palette-shell") ?? items[0];
    case "webhook_trigger":
      return items.find((item) => item.id === "palette-webhook") ?? items[0];
    default:
      return items.find((item) => item.id === "palette-shell") ?? items[0];
  }
};

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

const getUniqueNodeId = (
  baseId: string,
  existingNodes: readonly WorkflowExistingNode[],
) => {
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

export const createWorkflowNodeDraft: CreateWorkflowNodeDraft = (
  item: WorkflowPaletteItem,
  position,
  existingNodes,
) => {
  const baseNodeId = getPaletteBaseNodeId(item);
  const descriptor = item.nodeDescriptor;

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
    case "palette-db-query": {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("db_query");
      const subtitle = "新建数据库查询";

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
            kind: "db-query",
            nodeKey: nodeId,
            subtitle,
            title: "DB Query",
          },
        },
        panel,
      };
    }
    case "palette-set-state": {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("set_state");
      const subtitle = "设置工作流状态";

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
            kind: "set-state",
            nodeKey: nodeId,
            subtitle,
            title: "Set State",
          },
        },
        panel,
      };
    }
    case "palette-code": {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("code_node");
      const subtitle = "新建代码节点";

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
            accent: "#0F766E",
            icon: "code",
            kind: "effect",
            nodeKey: nodeId,
            subtitle,
            title: "Code",
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
      setSwitchBranches(panel, [
        { id: "branch-a", label: "then" },
        { id: "branch-b", label: "else" },
      ]);

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
            branchHandles: getBranchHandlesForNode("if-else", panel),
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
            branchHandles: getBranchHandlesForNode("switch", panel),
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
    case "palette-subflow": {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const panel = clonePanel("sub_workflow");
      const subtitle = "调用子工作流";

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
            kind: "sub-workflow",
            nodeKey: nodeId,
            subtitle,
            title: "Sub-Workflow",
          },
        },
        panel,
      };
    }
    default: {
      const nodeId = getUniqueNodeId(baseNodeId, existingNodes);
      const pluginSubtitle = `新建${item.label}`;

      if (descriptor && item.runnerType?.startsWith("plugin:")) {
        const panel = createPluginNodePanel(descriptor, nodeId, pluginSubtitle);

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
              runnerType: item.runnerType,
              subtitle: pluginSubtitle,
              title: item.label,
            },
          },
          panel,
        };
      }

      const panel = clonePanel("assign_task");
      const subtitle = `新建${item.label}`;
      const titleMap: Partial<Record<WorkflowPaletteItem["id"], string>> = {
        "palette-code": "Code",
        "palette-shell": "Shell",
        "palette-respond": "Respond",
        "palette-subflow": "Sub-Workflow",
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
              runnerType: item.runnerType,
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
            runnerType: item.runnerType,
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

export const normalizeWorkflowEdge = (edge: Edge): Edge => ({
  ...edge,
  type: WORKFLOW_EDGE_TYPE,
  style: {
    ...WORKFLOW_EDGE_STYLE,
    ...(edge.style &&
    typeof edge.style === "object" &&
    !Array.isArray(edge.style)
      ? edge.style
      : {}),
  },
});

export const normalizeWorkflowEdges = (edges: Edge[]): Edge[] =>
  edges.map((edge) => normalizeWorkflowEdge(edge));
