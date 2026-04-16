import type { Edge } from "@vue-flow/core";

import {
  getBranchHandlesForNode,
  type WorkflowFlowNode,
  type WorkflowNodeData,
  type WorkflowNodeKind,
  type WorkflowNodePanel,
  type WorkflowNodePosition,
  type WorkflowTabId,
  type WorkflowNodeType,
} from "./model";

export interface WorkflowExportOptions {
  selectedNodeId?: string;
  status: "draft" | "published";
  version: string;
  workflowId: string;
  workflowName: string;
}

export interface WorkflowExportFieldMap {
  [key: string]: string;
}

export interface WorkflowExportNode {
  config: Partial<Record<WorkflowTabId, WorkflowExportFieldMap>>;
  id: string;
  kind: WorkflowNodeKind;
  position: WorkflowNodePosition;
  status?: WorkflowNodeData["status"];
  subtitle?: WorkflowNodeData["subtitle"];
  title: WorkflowNodeData["title"];
  type: WorkflowNodeType;
}

export interface WorkflowExportEdge {
  id: string;
  label?: string;
  source: string;
  sourceHandle?: string | null;
  target: string;
  targetHandle?: string | null;
}

export interface WorkflowExportAnnotation {
  id: string;
  kind: "branch-label";
  position: WorkflowNodePosition;
  text: string;
}

export interface WorkflowExportDocument {
  editor: {
    annotations: WorkflowExportAnnotation[];
    selectedNodeId?: string;
  };
  exportedAt: string;
  graph: {
    edges: WorkflowExportEdge[];
    nodes: WorkflowExportNode[];
  };
  schemaVersion: "1.0";
  workflow: {
    id: string;
    name: string;
    status: "draft" | "published";
    version: string;
  };
}

const serializePanelConfig = (panel?: WorkflowNodePanel): Partial<Record<WorkflowTabId, WorkflowExportFieldMap>> => {
  if (!panel) {
    return {};
  }

  return panel.tabs.reduce<Partial<Record<WorkflowTabId, WorkflowExportFieldMap>>>((acc, tab) => {
    const fields = panel.fieldsByTab[tab] ?? [];

    acc[tab] = fields.reduce<WorkflowExportFieldMap>((fieldMap, field) => {
      fieldMap[field.key] = field.value;
      return fieldMap;
    }, {});

    return acc;
  }, {});
};

const createBranchLabelMap = (
  nodes: WorkflowFlowNode[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
) => {
  const labels = new Map<string, string>();

  nodes
    .filter((node) => node.data.kind !== "branch-label")
    .forEach((node) => {
      const branchHandles = getBranchHandlesForNode(
        node.data.kind,
        panelByNodeId[node.id],
      );

      branchHandles?.forEach((branch) => {
        labels.set(`${node.id}:${branch.id}`, branch.label);
      });
    });

  return labels;
};

export const createWorkflowExportDocument = (
  nodes: WorkflowFlowNode[],
  edges: Edge[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
  options: WorkflowExportOptions,
): WorkflowExportDocument => {
  const branchLabelMap = createBranchLabelMap(nodes, panelByNodeId);
  const annotations = nodes
    .filter((node) => node.data.kind === "branch-label")
    .map<WorkflowExportAnnotation>((node) => ({
      id: node.id,
      kind: "branch-label",
      position: node.position,
      text: node.data.title,
    }));

  const exportedNodes = nodes
    .filter((node) => node.data.kind !== "branch-label")
    .map<WorkflowExportNode>((node) => ({
      config: serializePanelConfig(panelByNodeId[node.id]),
      id: node.id,
      kind: node.data.kind,
      position: node.position,
      status: node.data.status,
      subtitle: node.data.subtitle,
      title: node.data.title,
      type: node.type,
    }));

  const exportedEdges = edges.map<WorkflowExportEdge>((edge) => ({
    id: edge.id,
    label:
      edge.sourceHandle && edge.source
        ? branchLabelMap.get(`${edge.source}:${edge.sourceHandle}`)
        : undefined,
    source: edge.source,
    sourceHandle: edge.sourceHandle,
    target: edge.target,
    targetHandle: edge.targetHandle,
  }));

  return {
    editor: {
      annotations,
      selectedNodeId: options.selectedNodeId,
    },
    exportedAt: new Date().toISOString(),
    graph: {
      edges: exportedEdges,
      nodes: exportedNodes,
    },
    schemaVersion: "1.0",
    workflow: {
      id: options.workflowId,
      name: options.workflowName,
      status: options.status,
      version: options.version,
    },
  };
};
