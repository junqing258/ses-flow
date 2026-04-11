import { describe, expect, it } from "vitest";

import { Position } from "@vue-flow/core";

import { createWorkflowEdges, createWorkflowPanels, type WorkflowFlowNode } from "@/features/workflow/model";
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
      kind: "action",
      nodeKey: "assign_task",
      subtitle: "分配分拣任务",
      title: "Action / Command",
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
      ["assign_task", "action"],
      ["wait_callback", "wait"],
      ["end_left", "end"],
      ["end_right", "end"],
    ]);

    const fetchNode = definition.nodes.find((node) => node.id === "fetch_order");
    expect(fetchNode?.inputMapping).toEqual({
      orderId: "{{trigger.body.orderId}}",
    });

    const switchNode = definition.nodes.find((node) => node.id === "switch_biz_type");
    expect(switchNode?.config).toEqual({
      expression: "{{input.bizType}}",
    });

    expect(definition.transitions).toEqual([
      { from: "start", to: "webhook_trigger" },
      { from: "webhook_trigger", to: "fetch_order" },
      { from: "fetch_order", to: "switch_biz_type" },
      { from: "switch_biz_type", to: "assign_task", label: "A", priority: 100 },
      { from: "switch_biz_type", to: "wait_callback", branchType: "default", priority: 1 },
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
      { from: "switch_biz_type", to: "wait_callback", label: "B", priority: 90 },
      { from: "assign_task", to: "end_left" },
      { from: "wait_callback", to: "end_right" },
    ]);
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
