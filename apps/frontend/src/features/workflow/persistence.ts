import type { Edge } from "@vue-flow/core";

import {
  WORKFLOW_PALETTE_CATEGORIES,
  createWorkflowEdges,
  createWorkflowNodeDraft,
  createWorkflowPanels,
  type WorkflowFlowNode,
  type WorkflowNodePanel,
  type WorkflowPaletteItem,
  type WorkflowTabId,
} from "./model";

export interface PersistedWorkflowDocument {
  editor: {
    activeTab?: WorkflowTabId;
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
  selectedNodeId?: string;
  status: "draft" | "published";
  version: string;
  workflowId: string;
  workflowName: string;
}

export interface WorkflowEditorState {
  activeTab: WorkflowTabId;
  edges: Edge[];
  nodes: WorkflowFlowNode[];
  panelByNodeId: Record<string, WorkflowNodePanel>;
  selectedNodeId: string;
}

const cloneNodeData = (data: WorkflowFlowNode["data"]): WorkflowFlowNode["data"] => ({
  active: data.active,
  accent: data.accent,
  icon: data.icon,
  kind: data.kind,
  nodeKey: data.nodeKey,
  status: data.status,
  subtitle: data.subtitle,
  title: data.title,
});

const cloneNodes = (nodes: WorkflowFlowNode[]): WorkflowFlowNode[] =>
  nodes.map((node) => ({
    data: cloneNodeData(node.data),
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

  return Object.entries(style).reduce<Record<string, string | number>>((accumulator, [key, value]) => {
    if (typeof value === "string" || typeof value === "number") {
      accumulator[key] = value;
    }

    return accumulator;
  }, {});
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

const clonePanels = (panelByNodeId: Record<string, WorkflowNodePanel>): Record<string, WorkflowNodePanel> =>
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
              type: field.type,
              value: field.value,
            })),
          ]),
        ),
        tabs: [...panel.tabs],
      } satisfies WorkflowNodePanel,
    ]),
  ) as Record<string, WorkflowNodePanel>;

const findPaletteItem = (paletteItemId: string): WorkflowPaletteItem => {
  const paletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap((category) => category.items).find((item) => item.id === paletteItemId);

  if (!paletteItem) {
    throw new Error(`Workflow palette item not found: ${paletteItemId}`);
  }

  return paletteItem;
};

export const createNewWorkflowEditorState = (): WorkflowEditorState => {
  const startDraft = createWorkflowNodeDraft(findPaletteItem("palette-start"), { x: 120, y: 260 }, []);
  const endDraft = createWorkflowNodeDraft(findPaletteItem("palette-end"), { x: 520, y: 260 }, [startDraft.node]);
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
        type: "smoothstep",
        style: {
          stroke: "#CBD5E1",
          strokeWidth: 2,
        },
      },
    ],
    nodes: [startNode, endNode],
    panelByNodeId: {
      [startNode.id]: startDraft.panel,
      [endNode.id]: endDraft.panel,
    },
    selectedNodeId: startNode.id,
  };
};

export const createInitialWorkflowEditorState = (): WorkflowEditorState => createNewWorkflowEditorState();

export const createPersistedWorkflowDocument = (
  nodes: WorkflowFlowNode[],
  edges: Edge[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
  options: PersistedWorkflowOptions,
): PersistedWorkflowDocument => ({
  editor: {
    activeTab: options.activeTab,
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

  return {
    activeTab: document.editor.activeTab ?? fallbackState.activeTab,
    edges: cloneEdges(document.graph.edges),
    nodes: cloneNodes(document.graph.nodes),
    panelByNodeId: clonePanels(document.graph.panels),
    selectedNodeId: document.editor.selectedNodeId ?? fallbackState.selectedNodeId,
  };
};
