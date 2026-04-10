import type { Edge } from "@vue-flow/core";

import {
  createWorkflowEdges,
  createWorkflowNodes,
  createWorkflowPanels,
  type WorkflowFlowNode,
  type WorkflowNodePanel,
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

export const createInitialWorkflowEditorState = (): WorkflowEditorState => ({
  activeTab: "base",
  edges: createWorkflowEdges(),
  nodes: createWorkflowNodes(),
  panelByNodeId: createWorkflowPanels(),
  selectedNodeId: "fetch_order",
});

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
