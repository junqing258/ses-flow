import type { Edge } from "@vue-flow/core";

import {
  WORKFLOW_EDGE_STYLE,
  WORKFLOW_EDGE_TYPE,
  WORKFLOW_PALETTE_CATEGORIES,
  createWorkflowNodeDraft,
  createWorkflowPanels,
  normalizeWorkflowEdges,
  type WorkflowFlowNode,
  type WorkflowNodeData,
  type WorkflowNodePanel,
  type WorkflowPaletteItem,
  type WorkflowTabId,
} from "./model";

export interface PersistedWorkflowDocument {
  editor: {
    activeTab?: WorkflowTabId;
    pageMode?: WorkflowPageMode;
    runDraft?: WorkflowRunDraft;
    selectedNodeId?: string;
  };
  graph: {
    edges: Edge[];
    nodes: WorkflowFlowNode[];
    panels: Record<string, WorkflowNodePanel>;
  };
  schemaVersion: "1.0";
  workflow: {
    id: string;
    name: string;
    status: "draft" | "published";
    version: string;
  };
}

interface PersistedWorkflowOptions {
  activeTab?: WorkflowTabId;
  pageMode?: WorkflowPageMode;
  runDraft?: WorkflowRunDraft;
  selectedNodeId?: string;
  status: "draft" | "published";
  version: string;
  workflowId: string;
  workflowName: string;
}

export type WorkflowPageMode = "edit" | "run" | "ai";

export interface WorkflowRunDraft {
  body: string;
  env: string;
  headers: string;
  triggerMode: "manual" | "webhook";
}

export interface WorkflowEditorState {
  activeTab: WorkflowTabId;
  edges: Edge[];
  nodes: WorkflowFlowNode[];
  panelByNodeId: Record<string, WorkflowNodePanel>;
  pageMode: WorkflowPageMode;
  runDraft: WorkflowRunDraft;
  selectedNodeId: string;
}

const createDefaultWorkflowRunDraft = (): WorkflowRunDraft => ({
  body: "{}",
  env: "{}",
  headers:
    '{\n  "x-request-id": "wf-run-demo-001",\n  "x-source": "workflow-editor"\n}',
  triggerMode: "manual",
});

const cloneRunDraft = (runDraft?: WorkflowRunDraft): WorkflowRunDraft => {
  const defaultRunDraft = createDefaultWorkflowRunDraft();

  return {
    body: runDraft?.body ?? defaultRunDraft.body,
    env: runDraft?.env ?? defaultRunDraft.env,
    headers: runDraft?.headers ?? defaultRunDraft.headers,
    triggerMode: runDraft?.triggerMode ?? defaultRunDraft.triggerMode,
  };
};

interface CloneNodeOptions {
  includeExecutionStatus?: boolean;
}

const cloneNodeData = (
  data: WorkflowNodeData,
  options: CloneNodeOptions = {},
): WorkflowNodeData => ({
  active: data.active,
  accent: data.accent,
  branchHandles: data.branchHandles?.map((branch) => ({
    id: branch.id,
    isDefault: branch.isDefault,
    label: branch.label,
  })),
  executionStatus: options.includeExecutionStatus
    ? data.executionStatus
    : undefined,
  icon: data.icon,
  kind: data.kind,
  nodeKey: data.nodeKey,
  runnerType: data.runnerType,
  status: data.status,
  subtitle: data.subtitle,
  title: data.title,
});

const cloneNodes = (
  nodes: WorkflowFlowNode[],
  options: CloneNodeOptions = {},
): WorkflowFlowNode[] =>
  nodes
    .filter((node) => node.data.kind !== "branch-label")
    .map((node) => ({
      data: cloneNodeData(node.data, options),
      deletable: node.deletable,
      draggable: node.draggable,
      id: node.id,
      parentNode: node.parentNode,
      position: {
        x: node.position.x,
        y: node.position.y,
      },
      selectable: node.selectable,
      sourcePosition: node.sourcePosition,
      targetPosition: node.targetPosition,
      type: node.type,
    })) as WorkflowFlowNode[];

const cloneEditorNodes = (
  nodes: WorkflowFlowNode[],
  options: CloneNodeOptions = {},
): WorkflowFlowNode[] =>
  nodes.map((node) => ({
    data: cloneNodeData(node.data, options),
    deletable: node.deletable,
    draggable: node.draggable,
    id: node.id,
    parentNode: node.parentNode,
    position: {
      x: node.position.x,
      y: node.position.y,
    },
    selectable: node.selectable,
    sourcePosition: node.sourcePosition,
    targetPosition: node.targetPosition,
    type: node.type,
  })) as WorkflowFlowNode[];

const cloneEdgeStyle = (style: Edge["style"]) => {
  if (!style || typeof style !== "object" || Array.isArray(style)) {
    return undefined;
  }

  return Object.entries(style).reduce<Record<string, string | number>>(
    (accumulator, [key, value]) => {
      if (typeof value === "string" || typeof value === "number") {
        accumulator[key] = value;
      }

      return accumulator;
    },
    {},
  );
};

const cloneEdges = (edges: Edge[]): Edge[] =>
  edges.map((edge) => ({
    animated: edge.animated,
    deletable: edge.deletable,
    id: edge.id,
    interactionWidth: edge.interactionWidth,
    label: edge.label,
    selectable: edge.selectable,
    source: edge.source,
    sourceHandle: edge.sourceHandle,
    style: cloneEdgeStyle(edge.style),
    target: edge.target,
    targetHandle: edge.targetHandle,
    type: edge.type,
    updatable: edge.updatable,
  }));

const clonePanels = (
  panelByNodeId: Record<string, WorkflowNodePanel>,
): Record<string, WorkflowNodePanel> =>
  Object.fromEntries(
    Object.entries(panelByNodeId).map(([nodeId, panel]) => [
      nodeId,
      {
        fieldsByTab: Object.fromEntries(
          Object.entries(panel.fieldsByTab).map(([tab, fields]) => [
            tab,
            (fields ?? []).map((field) => ({
              key: field.key,
              label: field.label,
              options: field.options?.map((option) => ({
                label: option.label,
                value: option.value,
              })),
              type: field.type,
              value: field.value,
            })),
          ]),
        ),
        tabs: [...panel.tabs],
      } satisfies WorkflowNodePanel,
    ]),
  ) as Record<string, WorkflowNodePanel>;

const ensureWebhookResponseModeField = (
  nodes: WorkflowFlowNode[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
): Record<string, WorkflowNodePanel> => {
  const defaults = createWorkflowPanels();
  const defaultField = defaults.webhook_trigger.fieldsByTab.base?.find(
    (field) => field.key === "responseMode",
  );

  if (!defaultField) {
    return panelByNodeId;
  }

  return Object.fromEntries(
    Object.entries(panelByNodeId).map(([nodeId, panel]) => {
      const node = nodes.find((item) => item.id === nodeId);

      if (node?.data.title !== "Webhook Trigger") {
        return [nodeId, panel] as const;
      }

      const baseFields = panel.fieldsByTab.base ?? [];
      const hasResponseMode = baseFields.some(
        (field) => field.key === "responseMode",
      );

      if (hasResponseMode) {
        return [nodeId, panel] as const;
      }

      const methodIndex = baseFields.findIndex(
        (field) => field.key === "method",
      );
      const nextBaseFields = [...baseFields];
      const insertAt =
        methodIndex >= 0 ? methodIndex + 1 : nextBaseFields.length;

      nextBaseFields.splice(insertAt, 0, {
        key: defaultField.key,
        label: defaultField.label,
        type: defaultField.type,
        value: defaultField.value,
      });

      return [
        nodeId,
        {
          ...panel,
          fieldsByTab: {
            ...panel.fieldsByTab,
            base: nextBaseFields,
          },
        },
      ] as const;
    }),
  ) as Record<string, WorkflowNodePanel>;
};

const ensureSubWorkflowSelectionField = (
  nodes: WorkflowFlowNode[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
): Record<string, WorkflowNodePanel> => {
  const defaults = createWorkflowPanels();
  const defaultField = defaults.sub_workflow.fieldsByTab.base?.find(
    (field) => field.key === "workflowRef",
  );

  if (!defaultField) {
    return panelByNodeId;
  }

  return Object.fromEntries(
    Object.entries(panelByNodeId).map(([nodeId, panel]) => {
      const node = nodes.find((item) => item.id === nodeId);

      if (
        node?.data.kind !== "sub-workflow" &&
        node?.data.title !== "Sub-Workflow"
      ) {
        return [nodeId, panel] as const;
      }

      const baseFields = panel.fieldsByTab.base ?? [];
      const existingWorkflowRef = baseFields.find(
        (field) => field.key === "workflowRef",
      );
      const legacyCommandField = baseFields.find(
        (field) => field.key === "command",
      );
      const workflowRefValue =
        existingWorkflowRef?.value ?? legacyCommandField?.value ?? "";
      const nextBaseFields = baseFields
        .filter((field) => field.key !== "command")
        .map((field) =>
          field.key === "workflowRef"
            ? {
                ...field,
                label: defaultField.label,
                type: defaultField.type,
                value: workflowRefValue,
              }
            : field,
        );

      if (!nextBaseFields.some((field) => field.key === "workflowRef")) {
        nextBaseFields.unshift({
          key: defaultField.key,
          label: defaultField.label,
          type: defaultField.type,
          value: workflowRefValue,
        });
      }

      return [
        nodeId,
        {
          ...panel,
          fieldsByTab: {
            ...panel.fieldsByTab,
            base: nextBaseFields,
          },
        },
      ] as const;
    }),
  ) as Record<string, WorkflowNodePanel>;
};

const ensureWaitMappingField = (
  nodes: WorkflowFlowNode[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
): Record<string, WorkflowNodePanel> => {
  const defaults = createWorkflowPanels();
  const defaultMappingField = defaults.wait_callback.fieldsByTab.mapping?.find(
    (field) => field.key === "payload",
  );
  const defaultCorrelationField = defaults.wait_callback.fieldsByTab.base?.find(
    (field) => field.key === "correlationKey",
  );

  if (!defaultMappingField || !defaultCorrelationField) {
    return panelByNodeId;
  }

  return Object.fromEntries(
    Object.entries(panelByNodeId).map(([nodeId, panel]) => {
      const node = nodes.find((item) => item.id === nodeId);

      if (node?.data.kind !== "wait") {
        return [nodeId, panel] as const;
      }

      const tabs = panel.tabs.includes("mapping")
        ? panel.tabs
        : [...panel.tabs, "mapping" as const];
      const baseFields = panel.fieldsByTab.base ?? [];
      const mappingFields = panel.fieldsByTab.mapping ?? [];
      const nextBaseFields = baseFields.some(
        (field) => field.key === "correlationKey",
      )
        ? baseFields
        : [
            ...baseFields,
            {
              key: defaultCorrelationField.key,
              label: defaultCorrelationField.label,
              type: defaultCorrelationField.type,
              value: defaultCorrelationField.value,
            },
          ];

      if (mappingFields.some((field) => field.key === "payload")) {
        return [
          nodeId,
          {
            ...panel,
            fieldsByTab: {
              ...panel.fieldsByTab,
              base: nextBaseFields,
            },
            tabs,
          },
        ] as const;
      }

      return [
        nodeId,
        {
          ...panel,
          fieldsByTab: {
            ...panel.fieldsByTab,
            mapping: [
              ...mappingFields,
              {
                key: defaultMappingField.key,
                label: defaultMappingField.label,
                type: defaultMappingField.type,
                value: defaultMappingField.value,
              },
            ],
            base: nextBaseFields,
          },
          tabs,
        },
      ] as const;
    }),
  ) as Record<string, WorkflowNodePanel>;
};

const findPaletteItem = (paletteItemId: string): WorkflowPaletteItem => {
  const paletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
    (category) => category.items,
  ).find((item) => item.id === paletteItemId);

  if (!paletteItem) {
    throw new Error(`Workflow palette item not found: ${paletteItemId}`);
  }

  return paletteItem;
};

export const createNewWorkflowEditorState = (): WorkflowEditorState => {
  const startDraft = createWorkflowNodeDraft(
    findPaletteItem("palette-start"),
    { x: 120, y: 260 },
    [],
  );
  const endDraft = createWorkflowNodeDraft(
    findPaletteItem("palette-end"),
    { x: 520, y: 260 },
    [startDraft.node],
  );
  const startNode: WorkflowFlowNode = {
    ...startDraft.node,
    data: {
      ...startDraft.node.data,
      active: true,
    },
  };
  const endNode: WorkflowFlowNode = {
    ...endDraft.node,
    data: {
      ...endDraft.node.data,
      active: false,
    },
  };

  return {
    activeTab: "base",
    edges: [
      {
        id: "edge:start:out->end:in",
        source: startNode.id,
        sourceHandle: "out",
        target: endNode.id,
        targetHandle: "in",
        type: WORKFLOW_EDGE_TYPE,
        style: WORKFLOW_EDGE_STYLE,
      },
    ],
    nodes: [startNode, endNode],
    panelByNodeId: {
      [startNode.id]: startDraft.panel,
      [endNode.id]: endDraft.panel,
    },
    pageMode: "edit",
    runDraft: createDefaultWorkflowRunDraft(),
    selectedNodeId: startNode.id,
  };
};

export const createInitialWorkflowEditorState = (): WorkflowEditorState =>
  createNewWorkflowEditorState();

export const clearWorkflowEditorSelection = (
  state: WorkflowEditorState,
): WorkflowEditorState => ({
  activeTab: state.activeTab,
  edges: cloneEdges(state.edges),
  nodes: cloneEditorNodes(state.nodes, { includeExecutionStatus: true }).map(
    (node) => ({
      ...node,
      data: {
        ...node.data,
        active: false,
      },
    }),
  ) as WorkflowFlowNode[],
  panelByNodeId: clonePanels(state.panelByNodeId),
  pageMode: state.pageMode,
  runDraft: cloneRunDraft(state.runDraft),
  selectedNodeId: "",
});

export const createPersistedWorkflowDocument = (
  nodes: WorkflowFlowNode[],
  edges: Edge[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
  options: PersistedWorkflowOptions,
): PersistedWorkflowDocument => ({
  editor: {
    activeTab: options.activeTab,
    pageMode: options.pageMode,
    runDraft: cloneRunDraft(options.runDraft),
    selectedNodeId: options.selectedNodeId,
  },
  graph: {
    edges: cloneEdges(edges),
    nodes: cloneNodes(nodes),
    panels: clonePanels(panelByNodeId),
  },
  schemaVersion: "1.0",
  workflow: {
    id: options.workflowId,
    name: options.workflowName,
    status: options.status,
    version: options.version,
  },
});

export const createWorkflowEditorStateFromDocument = (
  document: PersistedWorkflowDocument,
): WorkflowEditorState => {
  const fallbackState = createInitialWorkflowEditorState();
  const nodes = cloneNodes(document.graph.nodes).map((node) =>
    node.data.title === "Sub-Workflow" && node.data.kind === "trigger"
      ? {
          ...node,
          data: {
            ...node.data,
            kind: "sub-workflow",
          },
        }
      : node,
  ) as WorkflowFlowNode[];
  const panelByNodeId = ensureWaitMappingField(
    nodes,
    ensureSubWorkflowSelectionField(
      nodes,
      ensureWebhookResponseModeField(nodes, clonePanels(document.graph.panels)),
    ),
  );

  return {
    activeTab: document.editor.activeTab ?? fallbackState.activeTab,
    edges: normalizeWorkflowEdges(cloneEdges(document.graph.edges)),
    nodes,
    panelByNodeId,
    pageMode: document.editor.pageMode ?? fallbackState.pageMode,
    runDraft: cloneRunDraft(document.editor.runDraft),
    selectedNodeId:
      document.editor.selectedNodeId ?? fallbackState.selectedNodeId,
  };
};
