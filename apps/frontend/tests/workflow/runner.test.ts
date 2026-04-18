import { describe, expect, it } from "vitest";

import { Position } from "@vue-flow/core";

import {
  createWorkflowNodeDraft,
  createWorkflowEdges,
  createWorkflowPanels,
  setSwitchBranches,
  setSwitchFallbackHandle,
  type WorkflowFlowNode,
} from "@/features/workflow/model";
import {
  buildRunnerWorkflowDefinition,
  shouldPollWorkflowRunSummary,
} from "@/features/workflow/runner";

const createExampleWorkflowNodes = (): WorkflowFlowNode[] => [
  {
    id: "start",
    type: "terminal",
    position: { x: 56, y: 240 },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
    data: {
      accent: "#10B981",
      icon: "play",
      kind: "start",
      nodeKey: "start",
      title: "Start",
    },
  },
  {
    id: "webhook_trigger",
    type: "workflow-card",
    position: { x: 192, y: 176 },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
    data: {
      accent: "#6366F1",
      icon: "webhook",
      kind: "trigger",
      nodeKey: "webhook_trigger",
      subtitle: "接收入库订单",
      title: "Webhook Trigger",
    },
  },
  {
    id: "fetch_order",
    type: "workflow-card",
    position: { x: 520, y: 176 },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
    data: {
      accent: "#3B82F6",
      icon: "database",
      kind: "fetch",
      nodeKey: "fetch_order",
      subtitle: "查询订单",
      title: "Fetch",
    },
  },
  {
    id: "switch_biz_type",
    type: "workflow-card",
    position: { x: 848, y: 176 },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
    data: {
      accent: "#EC4899",
      icon: "gitBranch",
      kind: "switch",
      nodeKey: "switch_biz_type",
      subtitle: "业务分流",
      title: "Switch",
    },
  },
  {
    id: "branch_label_a",
    type: "branch-chip",
    position: { x: 1088, y: 134 },
    draggable: false,
    selectable: false,
    data: {
      accent: "#E5E7EB",
      icon: "gitBranch",
      kind: "branch-label",
      nodeKey: "branch_label_a",
      title: "bizType=A",
    },
  },
  {
    id: "branch_label_b",
    type: "branch-chip",
    position: { x: 1088, y: 308 },
    draggable: false,
    selectable: false,
    data: {
      accent: "#E5E7EB",
      icon: "gitBranch",
      kind: "branch-label",
      nodeKey: "branch_label_b",
      title: "bizType=B",
    },
  },
  {
    id: "assign_task",
    type: "workflow-card",
    position: { x: 1184, y: 88 },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
    data: {
      accent: "#F97316",
      icon: "zap",
      kind: "shell",
      nodeKey: "assign_task",
      subtitle: "分配分拣任务",
      title: "Shell",
    },
  },
  {
    id: "wait_callback",
    type: "workflow-card",
    position: { x: 1184, y: 262 },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
    data: {
      accent: "#F59E0B",
      icon: "clock",
      kind: "wait",
      nodeKey: "wait_callback",
      subtitle: "等待设备回调",
      title: "Wait",
    },
  },
  {
    id: "end_left",
    type: "terminal",
    position: { x: 1528, y: 95 },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
    data: {
      accent: "#EF4444",
      icon: "shield",
      kind: "end",
      nodeKey: "end_left",
      title: "End",
    },
  },
  {
    id: "end_right",
    type: "terminal",
    position: { x: 1528, y: 269 },
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
    data: {
      accent: "#EF4444",
      icon: "shield",
      kind: "end",
      nodeKey: "end_right",
      title: "End",
    },
  },
];

describe("buildRunnerWorkflowDefinition", () => {
  it("maps switch fallback branches into runner default transitions", () => {
    const definition = buildRunnerWorkflowDefinition(
      createExampleWorkflowNodes(),
      createWorkflowEdges(),
      createWorkflowPanels(),
      {
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
        workflowVersion: "v3",
      },
    );

    expect(definition.meta).toEqual({
      key: "sorting-main-flow",
      name: "sorting-main-flow",
      version: 3,
      status: "published",
    });

    expect(definition.trigger).toEqual({
      type: "webhook",
      path: "/api/workflow/inbound-order",
      responseMode: "async_ack",
    });

    expect(definition.nodes.map((node) => [node.id, node.type])).toEqual([
      ["start", "start"],
      ["webhook_trigger", "webhook_trigger"],
      ["fetch_order", "fetch"],
      ["switch_biz_type", "switch"],
      ["assign_task", "shell"],
      ["wait_callback", "wait"],
      ["end_left", "end"],
      ["end_right", "end"],
    ]);

    const fetchNode = definition.nodes.find(
      (node) => node.id === "fetch_order",
    );
    expect(fetchNode?.config).toEqual({
      method: "GET",
      url: "https://jsonplaceholder.typicode.com/todos",
      headers: {
        "x-source": "workflow-editor",
      },
    });
    expect(fetchNode?.inputMapping).toEqual({
      userId: "{{trigger.body.userId}}",
    });

    const switchNode = definition.nodes.find(
      (node) => node.id === "switch_biz_type",
    );
    expect(switchNode?.config).toEqual({
      expression: "{{input.bizType}}",
    });

    expect(definition.transitions).toEqual([
      { from: "start", to: "webhook_trigger" },
      { from: "webhook_trigger", to: "fetch_order" },
      { from: "fetch_order", to: "switch_biz_type" },
      { from: "switch_biz_type", to: "assign_task", label: "A", priority: 100 },
      {
        from: "switch_biz_type",
        to: "wait_callback",
        branchType: "default",
        priority: 1,
      },
      { from: "assign_task", to: "end_left" },
      { from: "wait_callback", to: "end_right" },
    ]);
  });

  it("keeps explicit switch branch labels when no fallback branch is configured", () => {
    const panels = createWorkflowPanels();
    const fallbackField = (panels.switch_biz_type.fieldsByTab.base ?? []).find(
      (field) => field.key === "fallback",
    );

    if (!fallbackField) {
      throw new Error("switch fallback field should exist");
    }

    fallbackField.value = "";

    const definition = buildRunnerWorkflowDefinition(
      createExampleWorkflowNodes(),
      createWorkflowEdges(),
      panels,
      {
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
        workflowVersion: "v3",
      },
    );

    expect(definition.transitions).toEqual([
      { from: "start", to: "webhook_trigger" },
      { from: "webhook_trigger", to: "fetch_order" },
      { from: "fetch_order", to: "switch_biz_type" },
      { from: "switch_biz_type", to: "assign_task", label: "A", priority: 100 },
      {
        from: "switch_biz_type",
        to: "wait_callback",
        label: "B",
        priority: 90,
      },
      { from: "assign_task", to: "end_left" },
      { from: "wait_callback", to: "end_right" },
    ]);
  });

  it("supports additional switch branches configured in edit mode", () => {
    const panels = createWorkflowPanels();
    const edges = [
      ...createWorkflowEdges(),
      {
        id: "switch->end-right-default",
        source: "switch_biz_type",
        sourceHandle: "branch-c",
        target: "end_right",
        targetHandle: "in",
        type: "smoothstep",
        style: {
          stroke: "#CBD5E1",
          strokeWidth: 2,
        },
      },
    ];

    setSwitchBranches(panels.switch_biz_type, [
      { id: "branch-a", label: "A" },
      { id: "branch-b", label: "B" },
      { id: "branch-c", label: "MANUAL" },
    ]);
    setSwitchFallbackHandle(panels.switch_biz_type, "branch-c");

    const definition = buildRunnerWorkflowDefinition(
      createExampleWorkflowNodes(),
      edges,
      panels,
      {
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
        workflowVersion: "v3",
      },
    );

    expect(definition.transitions).toHaveLength(8);
    expect(definition.transitions).toContainEqual({
      from: "switch_biz_type",
      to: "assign_task",
      label: "A",
      priority: 100,
    });
    expect(definition.transitions).toContainEqual({
      from: "switch_biz_type",
      to: "wait_callback",
      label: "B",
      priority: 90,
    });
    expect(definition.transitions).toContainEqual({
      from: "switch_biz_type",
      to: "end_right",
      branchType: "default",
      priority: 1,
    });
  });

  it("uses the configured webhook response mode in edit mode", () => {
    const panels = createWorkflowPanels();
    const responseModeField = panels.webhook_trigger.fieldsByTab.base?.find(
      (field) => field.key === "responseMode",
    );

    if (!responseModeField) {
      throw new Error("webhook response mode field should exist");
    }

    responseModeField.value = "sync";

    const definition = buildRunnerWorkflowDefinition(
      createExampleWorkflowNodes(),
      createWorkflowEdges(),
      panels,
      {
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
        workflowVersion: "v3",
      },
    );

    expect(definition.trigger).toEqual({
      type: "webhook",
      path: "/api/workflow/inbound-order",
      responseMode: "sync",
    });
  });

  it("exports code nodes with code-specific runner config", () => {
    const baseNodes = createExampleWorkflowNodes().filter(
      (node) => node.id !== "assign_task",
    );
    const { node: codeNode, panel: codePanel } = createWorkflowNodeDraft(
      {
        id: "palette-code",
        kind: "effect",
        label: "Code",
        icon: "code",
        accent: "#0F766E",
      },
      { x: 1184, y: 88 },
      baseNodes,
    );

    codeNode.id = "run_code";
    codeNode.data.nodeKey = "run_code";

    (codePanel.fieldsByTab.base ?? []).forEach((field) => {
      if (field.key === "nodeId") {
        field.value = "run_code";
      }
      if (field.key === "nodeName") {
        field.value = "执行 JavaScript";
      }
      if (field.key === "language") {
        field.value = "javascript";
      }
      if (field.key === "source") {
        field.value = "return { output: { normalizedQty: params.qty * 2 } };";
      }
    });

    const payloadField = (codePanel.fieldsByTab.mapping ?? []).find(
      (field) => field.key === "payload",
    );

    if (!payloadField) {
      throw new Error("code payload field should exist");
    }

    payloadField.value = "{\n  qty: input.qty\n}";

    const panels = createWorkflowPanels();
    panels.run_code = codePanel;

    const definition = buildRunnerWorkflowDefinition(
      [...baseNodes, codeNode],
      createWorkflowEdges().map((edge) =>
        edge.id === "switch->assign"
          ? {
              ...edge,
              id: "switch->code",
              target: "run_code",
            }
          : edge,
      ),
      panels,
      {
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
        workflowVersion: "v3",
      },
    );

    const codeDefinition = definition.nodes.find(
      (node) => node.id === "run_code",
    );

    expect(codeDefinition).toMatchObject({
      id: "run_code",
      name: "执行 JavaScript",
      type: "code",
      config: {
        language: "javascript",
        source: "return { output: { normalizedQty: params.qty * 2 } };",
      },
      inputMapping: {
        qty: "{{input.qty}}",
      },
    });
  });

  it("exports set-state nodes with state path and value mapping", () => {
    const baseNodes = createExampleWorkflowNodes().filter(
      (node) => node.id !== "assign_task",
    );
    const { node: setStateNode, panel: setStatePanel } = createWorkflowNodeDraft(
      {
        id: "palette-set-state",
        kind: "set-state",
        label: "Set State",
        icon: "database",
        accent: "#14B8A6",
      },
      { x: 1184, y: 88 },
      baseNodes,
    );

    setStateNode.id = "mark_decision";
    setStateNode.data.nodeKey = "mark_decision";

    (setStatePanel.fieldsByTab.base ?? []).forEach((field) => {
      if (field.key === "nodeId") {
        field.value = "mark_decision";
      }
      if (field.key === "nodeName") {
        field.value = "记录分流结果";
      }
      if (field.key === "statePath") {
        field.value = "decision";
      }
    });

    const valueField = (setStatePanel.fieldsByTab.mapping ?? []).find(
      (field) => field.key === "value",
    );

    if (!valueField) {
      throw new Error("set-state value field should exist");
    }

    valueField.value = "{\n  handledBy: input.route\n}";

    const panels = createWorkflowPanels();
    panels.mark_decision = setStatePanel;

    const definition = buildRunnerWorkflowDefinition(
      [...baseNodes, setStateNode],
      createWorkflowEdges().map((edge) =>
        edge.id === "switch->assign"
          ? {
              ...edge,
              id: "switch->set-state",
              target: "mark_decision",
            }
          : edge,
      ),
      panels,
      {
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
        workflowVersion: "v3",
      },
    );

    const setStateDefinition = definition.nodes.find(
      (node) => node.id === "mark_decision",
    );

    expect(setStateDefinition).toMatchObject({
      id: "mark_decision",
      name: "记录分流结果",
      type: "set_state",
      config: {
        path: "decision",
      },
      inputMapping: {
        value: {
          handledBy: "{{input.route}}",
        },
      },
    });
  });

  it("exports sub-workflow nodes with workflow references", () => {
    const baseNodes = createExampleWorkflowNodes().filter(
      (node) => node.id !== "assign_task",
    );
    const { node: subWorkflowNode, panel: subWorkflowPanel } =
      createWorkflowNodeDraft(
        {
          id: "palette-subflow",
          kind: "sub-workflow",
          label: "Sub-Workflow",
          icon: "webhook",
          accent: "#6366F1",
        },
        { x: 1184, y: 88 },
        baseNodes,
      );

    subWorkflowNode.id = "invoke_child_flow";
    subWorkflowNode.data.nodeKey = "invoke_child_flow";

    (subWorkflowPanel.fieldsByTab.base ?? []).forEach((field) => {
      if (field.key === "nodeId") {
        field.value = "invoke_child_flow";
      }
      if (field.key === "nodeName") {
        field.value = "调用分拣子流程";
      }
      if (field.key === "workflowRef") {
        field.value = "sorting-child-flow";
      }
    });

    const payloadField = (subWorkflowPanel.fieldsByTab.mapping ?? []).find(
      (field) => field.key === "payload",
    );

    if (!payloadField) {
      throw new Error("sub-workflow payload field should exist");
    }

    payloadField.value = "{\n  orderId: input.orderId\n}";

    const panels = createWorkflowPanels();
    panels.invoke_child_flow = subWorkflowPanel;

    const definition = buildRunnerWorkflowDefinition(
      [...baseNodes, subWorkflowNode],
      createWorkflowEdges().map((edge) =>
        edge.id === "switch->assign"
          ? {
              ...edge,
              id: "switch->sub-workflow",
              target: "invoke_child_flow",
            }
          : edge,
      ),
      panels,
      {
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
        workflowVersion: "v3",
      },
    );

    const subWorkflowDefinition = definition.nodes.find(
      (node) => node.id === "invoke_child_flow",
    );

    expect(subWorkflowDefinition).toMatchObject({
      id: "invoke_child_flow",
      name: "调用分拣子流程",
      type: "sub_workflow",
      config: {
        ref: "sorting-child-flow",
        workflowKey: "sorting-child-flow",
      },
      inputMapping: {
        orderId: "{{input.orderId}}",
      },
    });
  });
});

describe("shouldPollWorkflowRunSummary", () => {
  it("keeps polling while a run is active or waiting for resume", () => {
    expect(shouldPollWorkflowRunSummary("running")).toBe(true);
    expect(shouldPollWorkflowRunSummary("waiting")).toBe(true);
  });

  it("stops polling after a run reaches a terminal state", () => {
    expect(shouldPollWorkflowRunSummary("completed")).toBe(false);
    expect(shouldPollWorkflowRunSummary("failed")).toBe(false);
    expect(shouldPollWorkflowRunSummary("terminated")).toBe(false);
  });
});
