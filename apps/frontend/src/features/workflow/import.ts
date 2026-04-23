import { Position, type Edge } from "@vue-flow/core";

import {
  LEGACY_TASK_PALETTE_ITEM,
  WORKFLOW_EDGE_STYLE,
  WORKFLOW_EDGE_TYPE,
  WORKFLOW_PALETTE_CATEGORIES,
  createDefaultSwitchBranches,
  createSwitchBranchHandleId,
  createWorkflowPaletteItemMap,
  createWorkflowNodeDraft,
  getSwitchBranches,
  getSwitchFallbackHandle,
  resolvePaletteItemForRunnerType,
  setSwitchBranches,
  setSwitchFallbackHandle,
  syncBranchHandlesForNode,
  type WorkflowFlowNode,
  type WorkflowPaletteCategory,
  type WorkflowNodeKind,
  type WorkflowNodePanel,
  type WorkflowPaletteItem,
} from "./model";
import {
  createNewWorkflowEditorState,
  type WorkflowEditorState,
} from "./persistence";
import type { RunnerWorkflowDefinition } from "./runner";

const createStaticPaletteItemMap = () => ({
  ...createWorkflowPaletteItemMap(WORKFLOW_PALETTE_CATEGORIES),
  [LEGACY_TASK_PALETTE_ITEM.id]: LEGACY_TASK_PALETTE_ITEM,
});

const getPaletteIdForRunnerNodeType = (nodeType: string) => {
  if (nodeType.startsWith("plugin:")) {
    return `palette-${nodeType.replace(/^plugin:/, "").replace(/_/g, "-")}`;
  }

  switch (nodeType) {
    case "start":
      return "palette-start";
    case "end":
      return "palette-end";
    case "fetch":
      return "palette-fetch";
    case "set_state":
      return "palette-set-state";
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
    case "code":
      return "palette-code";
    case "shell":
      return "palette-shell";
    case "webhook_trigger":
      return "palette-webhook";
    default:
      return "palette-shell";
  }
};

const getNodeTitle = (nodeType: string) => {
  if (nodeType.startsWith("plugin:")) {
    return nodeType.replace(/^plugin:/, "");
  }

  switch (nodeType) {
    case "start":
      return "Start";
    case "end":
      return "End";
    case "fetch":
      return "Fetch";
    case "set_state":
      return "Set State";
    case "switch":
      return "Switch";
    case "if_else":
      return "If / Else";
    case "wait":
      return "Wait";
    case "shell":
      return "Shell";
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
      return "Shell";
  }
};

const getNodeKind = (nodeType: string): WorkflowNodeKind => {
  if (nodeType.startsWith("plugin:")) {
    return "effect";
  }

  switch (nodeType) {
    case "start":
      return "start";
    case "end":
      return "end";
    case "fetch":
      return "fetch";
    case "set_state":
      return "set-state";
    case "switch":
      return "switch";
    case "if_else":
      return "if-else";
    case "wait":
      return "wait";
    case "webhook_trigger":
      return "trigger";
    case "sub_workflow":
      return "sub-workflow";
    case "shell":
      return "shell";
    default:
      return "effect";
  }
};

const readEditorPosition = (
  node: RunnerWorkflowDefinition["nodes"][number],
  index: number,
) => {
  const editorPosition = node.annotations?.editorPosition;
  const maybePosition =
    editorPosition &&
    typeof editorPosition === "object" &&
    !Array.isArray(editorPosition)
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

const setPanelFieldValue = (
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

const serializeMappingValue = (value: unknown) => {
  if (value === undefined || value === null) {
    return "";
  }

  if (typeof value === "string") {
    return value;
  }

  return JSON.stringify(value, null, 2);
};

const clonePanel = (panel: WorkflowNodePanel): WorkflowNodePanel =>
  structuredClone(panel);

const readSetStateValue = (
  inputMapping: RunnerWorkflowDefinition["nodes"][number]["inputMapping"],
) => {
  if (
    inputMapping &&
    typeof inputMapping === "object" &&
    !Array.isArray(inputMapping) &&
    "value" in inputMapping
  ) {
    return inputMapping.value;
  }

  return inputMapping;
};

const readAnnotatedSwitchBranches = (
  node: RunnerWorkflowDefinition["nodes"][number] | undefined,
) => {
  const rawBranches = node?.annotations?.switchBranches;

  if (!Array.isArray(rawBranches)) {
    return [];
  }

  return rawBranches.flatMap((branch) => {
    if (!branch || typeof branch !== "object") {
      return [];
    }

    const branchRecord = branch as { id?: unknown; label?: unknown };

    if (
      typeof branchRecord.id !== "string" ||
      typeof branchRecord.label !== "string"
    ) {
      return [];
    }

    return [
      {
        id: branchRecord.id,
        label: branchRecord.label,
      },
    ];
  });
};

const readAnnotatedSwitchFallbackHandle = (
  node: RunnerWorkflowDefinition["nodes"][number] | undefined,
) =>
  typeof node?.annotations?.defaultBranchHandle === "string"
    ? node.annotations.defaultBranchHandle
    : "";

const createImportedNode = (
  definition: RunnerWorkflowDefinition,
  node: RunnerWorkflowDefinition["nodes"][number],
  index: number,
  existingNodes: WorkflowFlowNode[],
  paletteCategories: WorkflowPaletteCategory[],
) => {
  const paletteId = getPaletteIdForRunnerNodeType(node.type);
  const paletteItemById = {
    ...createWorkflowPaletteItemMap(paletteCategories),
    [LEGACY_TASK_PALETTE_ITEM.id]: LEGACY_TASK_PALETTE_ITEM,
  };
  const paletteItem =
    paletteItemById[paletteId] ??
    (node.type.startsWith("plugin:")
      ? resolvePaletteItemForRunnerType(node.type, paletteCategories)
      : createStaticPaletteItemMap()["palette-shell"]);
  const { node: draftNode, panel } = createWorkflowNodeDraft(
    paletteItem,
    readEditorPosition(node, index),
    existingNodes,
  );
  const nextPanel = clonePanel(panel);
  const nodeTitle = node.type.startsWith("plugin:")
    ? paletteItem.label
    : getNodeTitle(node.type);
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
      runnerType: node.type.startsWith("plugin:") ? node.type : draftNode.data.runnerType,
      subtitle: isTerminal ? undefined : node.name,
      title: nodeTitle,
    },
    type: isTerminal ? "terminal" : "workflow-card",
  };

  setPanelFieldValue(nextPanel, "nodeId", node.id);
  setPanelFieldValue(nextPanel, "nodeName", node.name);
  setPanelFieldValue(
    nextPanel,
    "timeout",
    node.timeoutMs ? String(node.timeoutMs) : "",
  );
  setPanelFieldValue(
    nextPanel,
    "note",
    typeof node.annotations?.note === "string" ? node.annotations.note : "",
  );

  if (node.type === "fetch") {
    setPanelFieldValue(
      nextPanel,
      "method",
      String(node.config?.method ?? "GET"),
    );
    setPanelFieldValue(nextPanel, "url", String(node.config?.url ?? ""));
    setPanelFieldValue(
      nextPanel,
      "headers",
      serializeMappingValue(node.config?.headers),
    );
    setPanelFieldValue(
      nextPanel,
      "inputFrom",
      serializeMappingValue(node.inputMapping),
    );
    setPanelFieldValue(
      nextPanel,
      "outputTo",
      serializeMappingValue(node.outputMapping),
    );
  }

  if (node.type === "set_state") {
    setPanelFieldValue(nextPanel, "statePath", String(node.config?.path ?? ""));
    setPanelFieldValue(
      nextPanel,
      "value",
      serializeMappingValue(readSetStateValue(node.inputMapping)),
    );
  }

  if (node.type === "switch" || node.type === "if_else") {
    setPanelFieldValue(
      nextPanel,
      "expression",
      String(node.config?.expression ?? ""),
    );
  }

  if (
    node.type === "shell" ||
    node.type === "task" ||
    node.type === "respond"
  ) {
    setPanelFieldValue(
      nextPanel,
      "command",
      String(
        node.config?.command ??
          node.config?.taskType ??
          node.config?.workflowKey ??
          "",
      ),
    );
    setPanelFieldValue(nextPanel, "shell", String(node.config?.shell ?? "sh"));
    setPanelFieldValue(
      nextPanel,
      "workingDirectory",
      String(node.config?.workingDirectory ?? node.config?.cwd ?? ""),
    );
    setPanelFieldValue(
      nextPanel,
      "payload",
      serializeMappingValue(node.inputMapping),
    );
  }

  if (node.type === "sub_workflow") {
    setPanelFieldValue(
      nextPanel,
      "workflowRef",
      String(node.config?.ref ?? node.config?.workflowKey ?? ""),
    );
    setPanelFieldValue(
      nextPanel,
      "payload",
      serializeMappingValue(node.inputMapping),
    );
  }

  if (node.type === "code") {
    setPanelFieldValue(
      nextPanel,
      "source",
      String(node.config?.source ?? node.config?.js ?? node.config?.code ?? ""),
    );
    setPanelFieldValue(
      nextPanel,
      "language",
      String(node.config?.language ?? node.config?.lang ?? "javascript"),
    );
    setPanelFieldValue(
      nextPanel,
      "payload",
      serializeMappingValue(node.inputMapping),
    );
  }

  if (node.type === "wait") {
    setPanelFieldValue(
      nextPanel,
      "waitEvent",
      String(node.config?.event ?? ""),
    );
  }

  if (node.type === "webhook_trigger") {
    setPanelFieldValue(
      nextPanel,
      "path",
      String(definition.trigger.path ?? ""),
    );
    setPanelFieldValue(
      nextPanel,
      "responseMode",
      String(definition.trigger.responseMode ?? "async_ack"),
    );
  }

  if (node.type.startsWith("plugin:")) {
    Object.entries(node.config ?? {}).forEach(([key, value]) => {
      setPanelFieldValue(nextPanel, `config:${key}`, serializeMappingValue(value));
      setPanelFieldValue(nextPanel, key, serializeMappingValue(value));
    });
    setPanelFieldValue(nextPanel, "runnerType", node.type);
    setPanelFieldValue(
      nextPanel,
      "payload",
      serializeMappingValue(node.inputMapping),
    );
  }

  if (node.retryPolicy?.max_attempts !== undefined) {
    setPanelFieldValue(
      nextPanel,
      "maxAttempts",
      String(node.retryPolicy.max_attempts),
    );
    setPanelFieldValue(
      nextPanel,
      "retryCount",
      String(node.retryPolicy.max_attempts),
    );
  }

  return {
    node: nextNode,
    panel: nextPanel,
  };
};

export const createWorkflowEditorStateFromRunnerDefinition = (
  definition: RunnerWorkflowDefinition,
  paletteCategories: WorkflowPaletteCategory[] = WORKFLOW_PALETTE_CATEGORIES,
): WorkflowEditorState => {
  const fallbackState = createNewWorkflowEditorState();
  const nodes: WorkflowFlowNode[] = [];
  const panelByNodeId: Record<string, WorkflowNodePanel> = {};
  const edges: Edge[] = [];

  definition.nodes.forEach((nodeDefinition, index) => {
    const imported = createImportedNode(
      definition,
      nodeDefinition,
      index,
      nodes,
      paletteCategories,
    );

    nodes.push(imported.node);
    panelByNodeId[imported.node.id] = imported.panel;
  });

  const nodeById = Object.fromEntries(
    nodes.map((node) => [node.id, node] as const),
  );
  const runnerNodeById = Object.fromEntries(
    definition.nodes.map((node) => [node.id, node] as const),
  );
  const transitionsBySource = definition.transitions.reduce<
    Record<string, RunnerWorkflowDefinition["transitions"]>
  >((accumulator, transition) => {
    accumulator[transition.from] = [
      ...(accumulator[transition.from] ?? []),
      transition,
    ];
    return accumulator;
  }, {});

  Object.entries(transitionsBySource).forEach(([sourceId, transitions]) => {
    const sourceNode = nodeById[sourceId];

    if (
      !sourceNode ||
      (sourceNode.data.kind !== "switch" && sourceNode.data.kind !== "if-else")
    ) {
      return;
    }

    const panel = panelByNodeId[sourceId];
    const labelledTransitions = transitions.filter(
      (transition) => transition.label,
    );
    const defaultTransition = transitions.find(
      (transition) => transition.branchType === "default",
    );

    if (sourceNode.data.kind === "if-else") {
      setPanelFieldValue(panel, "fallback", "else");
    }

    if (sourceNode.data.kind === "switch") {
      const annotatedBranches = readAnnotatedSwitchBranches(
        runnerNodeById[sourceId],
      );
      const annotatedFallbackHandle = readAnnotatedSwitchFallbackHandle(
        runnerNodeById[sourceId],
      );

      if (annotatedBranches.length > 0) {
        setSwitchBranches(panel, annotatedBranches);
        setSwitchFallbackHandle(
          panel,
          annotatedFallbackHandle ||
            annotatedBranches[annotatedBranches.length - 1]?.id ||
            "",
        );
        return;
      }

      const labels = labelledTransitions
        .map((transition) => transition.label?.trim() ?? "")
        .filter(
          (label, index, source) => label && source.indexOf(label) === index,
        );
      const branches =
        labels.length > 0
          ? labels.map((label, index) => ({
              id: createSwitchBranchHandleId(index),
              label,
            }))
          : createDefaultSwitchBranches();
      const fallbackHandle = defaultTransition
        ? defaultTransition.label
          ? (branches.find((branch) => branch.label === defaultTransition.label)
              ?.id ?? branches[branches.length - 1]?.id)
          : branches[branches.length - 1]?.id
        : "";

      setSwitchBranches(panel, branches);
      setSwitchFallbackHandle(panel, fallbackHandle ?? "");
    }
  });

  const usedSwitchHandlesBySource = new Map<string, Set<string>>();

  definition.transitions.forEach((transition, index) => {
    const sourceNode = nodeById[transition.from];
    let sourceHandle: string | undefined;

    if (sourceNode?.data.kind === "if-else") {
      sourceHandle = transition.label === "else" ? "branch-b" : "branch-a";
    } else if (sourceNode?.data.kind === "switch") {
      const switchBranches = getSwitchBranches(panelByNodeId[transition.from]);
      const fallbackHandle = getSwitchFallbackHandle(
        panelByNodeId[transition.from],
      );
      const usedHandles =
        usedSwitchHandlesBySource.get(transition.from) ?? new Set<string>();

      if (transition.branchType === "default") {
        sourceHandle =
          fallbackHandle ?? switchBranches[switchBranches.length - 1]?.id;
      } else if (transition.label) {
        sourceHandle = switchBranches.find(
          (branch) => branch.label === transition.label,
        )?.id;
      }

      if (!sourceHandle) {
        sourceHandle =
          switchBranches.find(
            (branch) =>
              branch.id !== fallbackHandle && !usedHandles.has(branch.id),
          )?.id ??
          switchBranches.find((branch) => !usedHandles.has(branch.id))?.id;
      }

      if (sourceHandle) {
        usedHandles.add(sourceHandle);
        usedSwitchHandlesBySource.set(transition.from, usedHandles);
      }
    } else if (sourceNode?.type !== "terminal") {
      sourceHandle = "out";
    }

    const targetNode = nodeById[transition.to];
    const targetHandle =
      targetNode?.type === "terminal" && targetNode.data.kind === "start"
        ? undefined
        : "in";

    edges.push({
      id: `edge:${transition.from}:${sourceHandle ?? "default"}->${transition.to}:${targetHandle ?? "default"}:${index}`,
      source: transition.from,
      sourceHandle,
      target: transition.to,
      targetHandle,
      type: WORKFLOW_EDGE_TYPE,
      style: WORKFLOW_EDGE_STYLE,
    });
  });

  const allNodes = nodes.map((node) =>
    syncBranchHandlesForNode(node, panelByNodeId[node.id]),
  );
  const selectedNodeId =
    nodes.find((node) => node.data.kind !== "branch-label")?.id ?? "";

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
