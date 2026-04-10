import { Position, type Edge } from "@vue-flow/core";

import {
  WORKFLOW_PALETTE_CATEGORIES,
  createWorkflowNodeDraft,
  type WorkflowFlowNode,
  type WorkflowNodePanel,
  type WorkflowPaletteItem,
} from "./model";
import { createNewWorkflowEditorState, type WorkflowEditorState } from "./persistence";
import type { RunnerWorkflowDefinition } from "./runner";

const paletteItemById = WORKFLOW_PALETTE_CATEGORIES.flatMap((category) => category.items).reduce<Record<string, WorkflowPaletteItem>>(
  (accumulator, item) => {
    accumulator[item.id] = item;
    return accumulator;
  },
  {},
);

const getPaletteIdForRunnerNodeType = (nodeType: string) => {
  switch (nodeType) {
    case "start":
      return "palette-start";
    case "end":
      return "palette-end";
    case "fetch":
      return "palette-fetch";
    case "switch":
      return "palette-switch";
    case "if_else":
      return "palette-if-else";
    case "wait":
      return "palette-wait";
    case "task":
      return "palette-task";
    case "respond":
      return "palette-respond";
    case "sub_workflow":
      return "palette-subflow";
    case "webhook_trigger":
      return "palette-webhook";
    default:
      return "palette-action";
  }
};

const getNodeTitle = (nodeType: string) => {
  switch (nodeType) {
    case "start":
      return "Start";
    case "end":
      return "End";
    case "fetch":
      return "Fetch";
    case "switch":
      return "Switch";
    case "if_else":
      return "If / Else";
    case "wait":
      return "Wait";
    case "task":
      return "Task";
    case "respond":
      return "Respond";
    case "sub_workflow":
      return "Sub-Workflow";
    case "webhook_trigger":
      return "Webhook Trigger";
    case "code":
      return "Code";
    default:
      return "Action / Command";
  }
};

const getNodeKind = (nodeType: string): WorkflowFlowNode["data"]["kind"] => {
  switch (nodeType) {
    case "start":
      return "start";
    case "end":
      return "end";
    case "fetch":
      return "fetch";
    case "switch":
      return "switch";
    case "if_else":
      return "if-else";
    case "wait":
      return "wait";
    case "webhook_trigger":
      return "trigger";
    default:
      return "action";
  }
};

const readEditorPosition = (node: RunnerWorkflowDefinition["nodes"][number], index: number) => {
  const editorPosition = node.annotations?.editorPosition;
  const maybePosition =
    editorPosition && typeof editorPosition === "object" && !Array.isArray(editorPosition)
      ? (editorPosition as { x?: unknown; y?: unknown })
      : null;

  if (
    maybePosition &&
    typeof maybePosition.x === "number" &&
    typeof maybePosition.y === "number"
  ) {
    return {
      x: maybePosition.x,
      y: maybePosition.y,
    };
  }

  return {
    x: 56 + index * 320,
    y: node.type === "start" || node.type === "end" ? 240 : 176,
  };
};

const setPanelFieldValue = (panel: WorkflowNodePanel, fieldKey: string, value: string) => {
  panel.tabs.forEach((tab) => {
    panel.fieldsByTab[tab]?.forEach((field) => {
      if (field.key === fieldKey) {
        field.value = value;
      }
    });
  });
};

const serializeMappingValue = (value: unknown) => {
  if (value === undefined || value === null) {
    return "";
  }

  if (typeof value === "string") {
    return value;
  }

  return JSON.stringify(value, null, 2);
};

const clonePanel = (panel: WorkflowNodePanel): WorkflowNodePanel => structuredClone(panel);

const createImportedNode = (
  definition: RunnerWorkflowDefinition,
  node: RunnerWorkflowDefinition["nodes"][number],
  index: number,
  existingNodes: WorkflowFlowNode[],
) => {
  const paletteId = getPaletteIdForRunnerNodeType(node.type);
  const paletteItem = paletteItemById[paletteId] ?? paletteItemById["palette-action"];
  const { node: draftNode, panel } = createWorkflowNodeDraft(
    paletteItem,
    readEditorPosition(node, index),
    existingNodes,
  );
  const nextPanel = clonePanel(panel);
  const nodeTitle = getNodeTitle(node.type);
  const isTerminal = node.type === "start" || node.type === "end";
  const nextNode: WorkflowFlowNode = {
    ...draftNode,
    id: node.id,
    position: readEditorPosition(node, index),
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
    data: {
      ...draftNode.data,
      kind: getNodeKind(node.type),
      nodeKey: node.id,
      subtitle: isTerminal ? undefined : node.name,
      title: nodeTitle,
    },
    type: isTerminal ? "terminal" : "workflow-card",
  };

  setPanelFieldValue(nextPanel, "nodeId", node.id);
  setPanelFieldValue(nextPanel, "nodeName", node.name);
  setPanelFieldValue(nextPanel, "timeout", node.timeoutMs ? String(node.timeoutMs) : "");
  setPanelFieldValue(nextPanel, "note", typeof node.annotations?.note === "string" ? node.annotations.note : "");

  if (node.type === "fetch") {
    setPanelFieldValue(nextPanel, "connector", String(node.config?.connector ?? ""));
    setPanelFieldValue(nextPanel, "inputFrom", serializeMappingValue(node.inputMapping));
    setPanelFieldValue(nextPanel, "outputTo", serializeMappingValue(node.outputMapping));
  }

  if (node.type === "switch" || node.type === "if_else") {
    setPanelFieldValue(nextPanel, "expression", String(node.config?.expression ?? ""));
  }

  if (node.type === "action" || node.type === "task" || node.type === "respond" || node.type === "sub_workflow" || node.type === "code") {
    setPanelFieldValue(nextPanel, "command", String(node.config?.action ?? node.config?.taskType ?? node.config?.workflowKey ?? ""));
    setPanelFieldValue(nextPanel, "payload", serializeMappingValue(node.inputMapping));
  }

  if (node.type === "wait") {
    setPanelFieldValue(nextPanel, "waitEvent", String(node.config?.event ?? ""));
  }

  if (node.type === "webhook_trigger") {
    setPanelFieldValue(nextPanel, "path", String(definition.trigger.path ?? ""));
  }

  if (node.retryPolicy?.max_attempts !== undefined) {
    setPanelFieldValue(nextPanel, "maxAttempts", String(node.retryPolicy.max_attempts));
    setPanelFieldValue(nextPanel, "retryCount", String(node.retryPolicy.max_attempts));
  }

  return {
    node: nextNode,
    panel: nextPanel,
  };
};

const branchLabelPosition = (sourceNode: WorkflowFlowNode, branch: "branch-a" | "branch-b") => ({
  x: sourceNode.position.x + 240,
  y: branch === "branch-a" ? sourceNode.position.y - 42 : sourceNode.position.y + 132,
});

const createBranchLabelNode = (
  sourceNode: WorkflowFlowNode,
  branch: "branch-a" | "branch-b",
  label: string,
): WorkflowFlowNode => ({
  id: `${sourceNode.id}_${branch}_label`,
  type: "branch-chip",
  position: branchLabelPosition(sourceNode, branch),
  draggable: false,
  selectable: false,
  data: {
    accent: "#E5E7EB",
    icon: "gitBranch",
    kind: "branch-label",
    nodeKey: `${sourceNode.id}_${branch}_label`,
    title: label,
  },
});

export const createWorkflowEditorStateFromRunnerDefinition = (
  definition: RunnerWorkflowDefinition,
): WorkflowEditorState => {
  const fallbackState = createNewWorkflowEditorState();
  const nodes: WorkflowFlowNode[] = [];
  const panelByNodeId: Record<string, WorkflowNodePanel> = {};
  const edges: Edge[] = [];
  const branchLabelNodes: WorkflowFlowNode[] = [];

  definition.nodes.forEach((nodeDefinition, index) => {
    const imported = createImportedNode(definition, nodeDefinition, index, nodes);

    nodes.push(imported.node);
    panelByNodeId[imported.node.id] = imported.panel;
  });

  const nodeById = Object.fromEntries(nodes.map((node) => [node.id, node] as const));
  const transitionsBySource = definition.transitions.reduce<Record<string, RunnerWorkflowDefinition["transitions"]>>((accumulator, transition) => {
    accumulator[transition.from] = [...(accumulator[transition.from] ?? []), transition];
    return accumulator;
  }, {});

  Object.entries(transitionsBySource).forEach(([sourceId, transitions]) => {
    const sourceNode = nodeById[sourceId];

    if (!sourceNode || (sourceNode.data.kind !== "switch" && sourceNode.data.kind !== "if-else")) {
      return;
    }

    const panel = panelByNodeId[sourceId];
    const labelledTransitions = transitions.filter((transition) => transition.label);
    const defaultTransition = transitions.find((transition) => transition.branchType === "default");

    if (sourceNode.data.kind === "if-else") {
      const thenLabel = labelledTransitions.find((transition) => transition.label === "then")?.label ?? "then";
      const elseLabel = labelledTransitions.find((transition) => transition.label === "else")?.label ?? "else";

      setPanelFieldValue(panel, "caseA", thenLabel);
      setPanelFieldValue(panel, "caseB", elseLabel);
    }

    if (sourceNode.data.kind === "switch") {
      const expression = panel.fieldsByTab.base?.find((field) => field.key === "expression")?.value || "value";
      const firstLabel = labelledTransitions[0]?.label ?? "A";
      const secondLabel = labelledTransitions[1]?.label ?? defaultTransition?.label ?? "B";

      setPanelFieldValue(panel, "caseA", `${expression} === '${firstLabel}'`);
      setPanelFieldValue(panel, "caseB", `${expression} === '${secondLabel}'`);
      setPanelFieldValue(panel, "fallback", defaultTransition?.label ?? "default");
    }
  });

  definition.transitions.forEach((transition, index) => {
    const sourceNode = nodeById[transition.from];
    let sourceHandle: string | undefined;

    if (sourceNode?.data.kind === "if-else") {
      sourceHandle = transition.label === "else" ? "branch-b" : "branch-a";
    } else if (sourceNode?.data.kind === "switch") {
      sourceHandle = transition.branchType === "default" ? "branch-b" : edges.some((edge) => edge.source === transition.from && edge.sourceHandle === "branch-a") ? "branch-b" : "branch-a";
    } else if (sourceNode?.type !== "terminal") {
      sourceHandle = "out";
    }

    const targetNode = nodeById[transition.to];
    const targetHandle = targetNode?.type === "terminal" && targetNode.data.kind === "start" ? undefined : "in";

    edges.push({
      id: `edge:${transition.from}:${sourceHandle ?? "default"}->${transition.to}:${targetHandle ?? "default"}:${index}`,
      source: transition.from,
      sourceHandle,
      target: transition.to,
      targetHandle,
      type: "smoothstep",
      style: {
        stroke: "#CBD5E1",
        strokeWidth: 2,
      },
    });

    if (sourceNode?.data.kind === "switch" || sourceNode?.data.kind === "if-else") {
      if (sourceHandle === "branch-a" && !branchLabelNodes.some((node) => node.id === `${sourceNode.id}_branch-a_label`)) {
        branchLabelNodes.push(createBranchLabelNode(sourceNode, "branch-a", transition.label ?? "A"));
      }

      if (sourceHandle === "branch-b" && !branchLabelNodes.some((node) => node.id === `${sourceNode.id}_branch-b_label`)) {
        branchLabelNodes.push(createBranchLabelNode(sourceNode, "branch-b", transition.label ?? transition.branchType ?? "B"));
      }
    }
  });

  const allNodes = [...nodes, ...branchLabelNodes];
  const selectedNodeId = nodes.find((node) => node.data.kind !== "branch-label")?.id ?? "";

  return {
    activeTab: "base",
    edges,
    nodes: allNodes.map((node) => ({
      ...node,
      data: {
        ...node.data,
        active: node.id === selectedNodeId,
      },
    })) as WorkflowFlowNode[],
    panelByNodeId,
    pageMode: fallbackState.pageMode,
    runDraft: fallbackState.runDraft,
    selectedNodeId,
  };
};
