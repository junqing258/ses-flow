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

const cloneValue = <T>(value: T): T => structuredClone(value);

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
    edges: cloneValue(edges),
    nodes: cloneValue(nodes),
    panels: cloneValue(panelByNodeId),
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
    edges: cloneValue(document.graph.edges),
    nodes: cloneValue(document.graph.nodes),
    panelByNodeId: cloneValue(document.graph.panels),
    selectedNodeId: document.editor.selectedNodeId ?? fallbackState.selectedNodeId,
  };
};
