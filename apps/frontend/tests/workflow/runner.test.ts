import { describe, expect, it } from "vitest";

import { createWorkflowEdges, createWorkflowNodes, createWorkflowPanels } from "@/features/workflow/model";
import { buildRunnerWorkflowDefinition } from "@/features/workflow/runner";

describe("buildRunnerWorkflowDefinition", () => {
  it("maps the editor workflow graph into runner definition payload", () => {
    const definition = buildRunnerWorkflowDefinition(
      createWorkflowNodes(),
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
      { from: "switch_biz_type", to: "wait_callback", label: "B", priority: 90 },
      { from: "assign_task", to: "end_left" },
      { from: "wait_callback", to: "end_right" },
    ]);
  });
});
