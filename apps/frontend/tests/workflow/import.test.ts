import { describe, expect, it } from "vitest";

import { createWorkflowEditorStateFromRunnerDefinition } from "@/features/workflow/import";
import type { RunnerWorkflowDefinition } from "@/features/workflow/runner";

describe("createWorkflowEditorStateFromRunnerDefinition", () => {
  it("restores webhook response mode into the editor panel", () => {
    const definition: RunnerWorkflowDefinition = {
      meta: {
        key: "coverage-flow",
        name: "Coverage Flow",
        version: 1,
        status: "published",
      },
      trigger: {
        type: "webhook",
        path: "/flows/coverage",
        responseMode: "sync",
      },
      inputSchema: {
        type: "object",
      },
      nodes: [
        {
          id: "start_1",
          type: "start",
          name: "Start",
        },
        {
          id: "webhook_in",
          type: "webhook_trigger",
          name: "Webhook Trigger",
          config: {
            mode: "full",
          },
        },
        {
          id: "end_1",
          type: "end",
          name: "End",
        },
      ],
      transitions: [
        { from: "start_1", to: "webhook_in" },
        { from: "webhook_in", to: "end_1" },
      ],
      policies: {
        allowManualRetry: true,
      },
    };

    const state = createWorkflowEditorStateFromRunnerDefinition(definition);
    const webhookPanel = state.panelByNodeId.webhook_in;

    expect(
      webhookPanel.fieldsByTab.base?.find((field) => field.key === "path")?.value,
    ).toBe("/flows/coverage");
    expect(
      webhookPanel.fieldsByTab.base?.find((field) => field.key === "responseMode")
        ?.value,
    ).toBe("sync");
  });
});
