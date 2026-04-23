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
      webhookPanel.fieldsByTab.base?.find((field) => field.key === "path")
        ?.value,
    ).toBe("/flows/coverage");
    expect(
      webhookPanel.fieldsByTab.base?.find(
        (field) => field.key === "responseMode",
      )?.value,
    ).toBe("sync");
  });

  it("restores sub-workflow references into the workflow selector field", () => {
    const definition: RunnerWorkflowDefinition = {
      meta: {
        key: "coverage-flow",
        name: "Coverage Flow",
        version: 1,
        status: "published",
      },
      trigger: {
        type: "manual",
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
          id: "invoke_child",
          type: "sub_workflow",
          name: "Invoke Child",
          config: {
            ref: "child-flow",
          },
          inputMapping: {
            orderId: "{{input.orderId}}",
          },
        },
        {
          id: "end_1",
          type: "end",
          name: "End",
        },
      ],
      transitions: [
        { from: "start_1", to: "invoke_child" },
        { from: "invoke_child", to: "end_1" },
      ],
      policies: {
        allowManualRetry: true,
      },
    };

    const state = createWorkflowEditorStateFromRunnerDefinition(definition);
    const subWorkflowPanel = state.panelByNodeId.invoke_child;

    expect(
      subWorkflowPanel.fieldsByTab.base?.find(
        (field) => field.key === "workflowRef",
      )?.value,
    ).toBe("child-flow");
    expect(
      state.nodes.find((node) => node.id === "invoke_child")?.data.kind,
    ).toBe("sub-workflow");
  });

  it("restores set-state config into editor fields", () => {
    const definition: RunnerWorkflowDefinition = {
      meta: {
        key: "coverage-flow",
        name: "Coverage Flow",
        version: 1,
        status: "published",
      },
      trigger: {
        type: "manual",
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
          id: "mark_decision",
          type: "set_state",
          name: "Mark Decision",
          config: {
            path: "decision",
          },
          inputMapping: {
            value: {
              handledBy: "{{input.route}}",
            },
          },
        },
        {
          id: "end_1",
          type: "end",
          name: "End",
        },
      ],
      transitions: [
        { from: "start_1", to: "mark_decision" },
        { from: "mark_decision", to: "end_1" },
      ],
      policies: {
        allowManualRetry: true,
      },
    };

    const state = createWorkflowEditorStateFromRunnerDefinition(definition);
    const setStatePanel = state.panelByNodeId.mark_decision;

    expect(
      setStatePanel.fieldsByTab.base?.find((field) => field.key === "statePath")
        ?.value,
    ).toBe("decision");
    expect(
      setStatePanel.fieldsByTab.mapping?.find((field) => field.key === "value")
        ?.value,
    ).toBe('{\n  "handledBy": "{{input.route}}"\n}');
    expect(
      state.nodes.find((node) => node.id === "mark_decision")?.data.kind,
    ).toBe("set-state");
  });

  it("keeps legacy task nodes readable in the editor", () => {
    const definition: RunnerWorkflowDefinition = {
      meta: {
        key: "legacy-task-flow",
        name: "Legacy Task Flow",
        version: 1,
        status: "published",
      },
      trigger: {
        type: "manual",
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
          id: "manual_review",
          type: "task",
          name: "Manual Review",
          config: {
            taskType: "manual.review",
          },
          inputMapping: {
            orderId: "{{input.orderId}}",
          },
        },
        {
          id: "end_1",
          type: "end",
          name: "End",
        },
      ],
      transitions: [
        { from: "start_1", to: "manual_review" },
        { from: "manual_review", to: "end_1" },
      ],
      policies: {
        allowManualRetry: true,
      },
    };

    const state = createWorkflowEditorStateFromRunnerDefinition(definition);
    const taskPanel = state.panelByNodeId.manual_review;

    expect(
      state.nodes.find((node) => node.id === "manual_review")?.data.title,
    ).toBe("Task");
    expect(
      taskPanel.fieldsByTab.base?.find((field) => field.key === "command")
        ?.value,
    ).toBe("manual.review");
    expect(
      taskPanel.fieldsByTab.mapping?.find((field) => field.key === "payload")
        ?.value,
    ).toBe('{\n  "orderId": "{{input.orderId}}"\n}');
  });

  it("restores plugin nodes from dynamic descriptors", () => {
    const definition: RunnerWorkflowDefinition = {
      meta: {
        key: "plugin-flow",
        name: "Plugin Flow",
        version: 1,
        status: "published",
      },
      trigger: {
        type: "manual",
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
          id: "hello_world_1",
          type: "plugin:hello_world",
          name: "Say Hello",
          config: {
            target: "SES",
            prefix: "Hi",
          },
          inputMapping: {
            name: "{{input.name}}",
          },
        },
        {
          id: "end_1",
          type: "end",
          name: "End",
        },
      ],
      transitions: [
        { from: "start_1", to: "hello_world_1" },
        { from: "hello_world_1", to: "end_1" },
      ],
      policies: {
        allowManualRetry: true,
      },
    };

    const state = createWorkflowEditorStateFromRunnerDefinition(definition, [
      {
        id: "dynamic-biz",
        label: "业务节点",
        icon: "activity",
        defaultOpen: false,
        items: [
          {
            id: "palette-hello-world",
            kind: "effect",
            label: "Hello World",
            icon: "activity",
            accent: "#0EA5E9",
            runnerType: "plugin:hello_world",
            nodeDescriptor: {
              id: "hello_world",
              kind: "effect",
              runnerType: "plugin:hello_world",
              version: "1.0.0",
              category: "业务节点",
              displayName: "Hello World",
              status: "stable",
              transport: "http",
              timeoutMs: 5000,
              configSchema: {
                type: "object",
                properties: {
                  target: { type: "string", title: "默认问候对象" },
                  prefix: { type: "string", title: "问候前缀" },
                },
              },
              defaults: {
                target: "World",
                prefix: "Hello",
              },
            },
          },
        ],
      },
    ]);
    const pluginNode = state.nodes.find((node) => node.id === "hello_world_1");
    const pluginPanel = state.panelByNodeId.hello_world_1;

    expect(pluginNode?.data.title).toBe("Hello World");
    expect(pluginNode?.data.runnerType).toBe("plugin:hello_world");
    expect(
      pluginPanel.fieldsByTab.base?.find((field) => field.key === "runnerType")
        ?.value,
    ).toBe("plugin:hello_world");
    expect(
      pluginPanel.fieldsByTab.base?.find((field) => field.key === "config:target")
        ?.value,
    ).toBe("SES");
    expect(
      pluginPanel.fieldsByTab.mapping?.find((field) => field.key === "payload")
        ?.value,
    ).toBe('{\n  "name": "{{input.name}}"\n}');
  });
});
