import { describe, expect, it } from "vitest";
import { Position } from "@vue-flow/core";

import {
  clearWorkflowEditorSelection,
  createNewWorkflowEditorState,
  createPersistedWorkflowDocument,
  createWorkflowEditorStateFromDocument,
} from "@/features/workflow/persistence";

describe("workflow persistence", () => {
  it("initializes new workflows with empty trigger body and run env", () => {
    const state = createNewWorkflowEditorState();

    expect(state.runDraft.body).toBe("{}");
    expect(state.runDraft.env).toBe("{}");
  });

  it("clears the default node selection for edit-mode entry", () => {
    const state = createNewWorkflowEditorState();

    const clearedState = clearWorkflowEditorSelection(state);

    expect(clearedState.selectedNodeId).toBe("");
    expect(clearedState.nodes.every((node) => node.data.active === false)).toBe(
      true,
    );
    expect(clearedState.runDraft).toEqual(state.runDraft);
  });

  it("persists run mode and run draft fields", () => {
    const state = createNewWorkflowEditorState();
    const runDraft = {
      body: '{\n  "orderId": "SO-20002"\n}',
      env: '{\n  "tenantId": "tenant-b",\n  "warehouseId": "WHS-HZ-01"\n}',
      headers: '{\n  "x-request-id": "wf-run-002"\n}',
      triggerMode: "webhook" as const,
    };

    const document = createPersistedWorkflowDocument(
      state.nodes,
      state.edges,
      state.panelByNodeId,
      {
        activeTab: state.activeTab,
        pageMode: "run",
        runDraft,
        selectedNodeId: state.selectedNodeId,
        status: "draft",
        version: "v3",
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
      },
    );

    expect(document.editor.pageMode).toBe("run");
    expect(document.editor.runDraft).toEqual(runDraft);

    const restored = createWorkflowEditorStateFromDocument(document);

    expect(restored.pageMode).toBe("run");
    expect(restored.runDraft).toEqual(runDraft);
  });

  it("falls back to default run state for older documents", () => {
    const state = createNewWorkflowEditorState();
    const document = createPersistedWorkflowDocument(
      state.nodes,
      state.edges,
      state.panelByNodeId,
      {
        activeTab: state.activeTab,
        selectedNodeId: state.selectedNodeId,
        status: "draft",
        version: "v3",
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
      },
    );

    delete document.editor.pageMode;
    delete document.editor.runDraft;

    const restored = createWorkflowEditorStateFromDocument(document);

    expect(restored.pageMode).toBe("edit");
    expect(restored.runDraft.triggerMode).toBe("manual");
    expect(restored.runDraft.body).toBe("{}");
    expect(restored.runDraft.env).toBe("{}");
  });

  it("does not persist runtime execution status into workflow documents", () => {
    const state = createNewWorkflowEditorState();

    state.nodes[0]!.data.executionStatus = "success";

    const document = createPersistedWorkflowDocument(
      state.nodes,
      state.edges,
      state.panelByNodeId,
      {
        activeTab: state.activeTab,
        selectedNodeId: state.selectedNodeId,
        status: "draft",
        version: "v3",
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
      },
    );

    expect(document.graph.nodes[0]?.data.executionStatus).toBeUndefined();
  });

  it("clears historical execution status when restoring older documents", () => {
    const state = createNewWorkflowEditorState();
    const document = createPersistedWorkflowDocument(
      state.nodes,
      state.edges,
      state.panelByNodeId,
      {
        activeTab: state.activeTab,
        selectedNodeId: state.selectedNodeId,
        status: "draft",
        version: "v3",
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
      },
    );

    document.graph.nodes[0]!.data.executionStatus = "success";

    const restored = createWorkflowEditorStateFromDocument(document);

    expect(restored.nodes[0]?.data.executionStatus).toBeUndefined();
  });

  it("hydrates missing webhook response mode for older editor documents", () => {
    const state = createNewWorkflowEditorState();
    const webhookNode = {
      id: "webhook_trigger",
      type: "workflow-card" as const,
      position: { x: 192, y: 176 },
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      data: {
        accent: "#6366F1",
        icon: "webhook" as const,
        kind: "trigger" as const,
        nodeKey: "webhook_trigger",
        subtitle: "接收入库订单",
        title: "Webhook Trigger",
      },
    };

    const document = createPersistedWorkflowDocument(
      [state.nodes[0]!, webhookNode, state.nodes[1]!],
      state.edges,
      {
        ...state.panelByNodeId,
        webhook_trigger: {
          tabs: ["base", "mapping", "error"],
          fieldsByTab: {
            base: [
              {
                key: "path",
                label: "Webhook Path",
                type: "input",
                value: "/api/workflow/inbound-order",
              },
              {
                key: "nodeName",
                label: "节点名称",
                type: "input",
                value: "接收入库订单",
              },
              {
                key: "method",
                label: "请求方式",
                type: "select",
                value: "POST",
              },
              {
                key: "nodeId",
                label: "节点 ID",
                type: "readonly",
                value: "webhook_trigger",
              },
            ],
            mapping: [
              {
                key: "payload",
                label: "原始载荷",
                type: "textarea",
                value: "{\n  orderId: body.orderId\n}",
              },
            ],
            error: [
              {
                key: "onInvalid",
                label: "签名失败处理",
                type: "select",
                value: "reject_401",
              },
            ],
          },
        },
      },
      {
        activeTab: state.activeTab,
        selectedNodeId: "webhook_trigger",
        status: "draft",
        version: "v3",
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
      },
    );

    const restored = createWorkflowEditorStateFromDocument(document);
    const responseModeField =
      restored.panelByNodeId.webhook_trigger.fieldsByTab.base?.find(
        (field) => field.key === "responseMode",
      );

    expect(responseModeField).toBeDefined();
    expect(responseModeField?.value).toBe("async_ack");
  });

  it("migrates legacy sub-workflow panels from command input to workflow selector", () => {
    const state = createNewWorkflowEditorState();
    const subWorkflowNode = {
      id: "invoke_child",
      type: "workflow-card" as const,
      position: { x: 192, y: 176 },
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      data: {
        accent: "#6366F1",
        icon: "webhook" as const,
        kind: "trigger" as const,
        nodeKey: "invoke_child",
        subtitle: "调用子工作流",
        title: "Sub-Workflow",
      },
    };

    const document = createPersistedWorkflowDocument(
      [state.nodes[0]!, subWorkflowNode, state.nodes[1]!],
      state.edges,
      {
        ...state.panelByNodeId,
        invoke_child: {
          tabs: ["base", "mapping", "retry"],
          fieldsByTab: {
            base: [
              {
                key: "command",
                label: "命令名称",
                type: "input",
                value: "child-flow",
              },
              {
                key: "nodeName",
                label: "节点名称",
                type: "input",
                value: "调用子工作流",
              },
              {
                key: "nodeId",
                label: "节点 ID",
                type: "readonly",
                value: "invoke_child",
              },
            ],
            mapping: [
              {
                key: "payload",
                label: "载荷",
                type: "textarea",
                value: "{\n  orderId: input.orderId\n}",
              },
            ],
            retry: [],
          },
        },
      },
      {
        activeTab: state.activeTab,
        selectedNodeId: "invoke_child",
        status: "draft",
        version: "v3",
        workflowId: "sorting-main-flow",
        workflowName: "sorting-main-flow",
      },
    );

    const restored = createWorkflowEditorStateFromDocument(document);
    const baseFields =
      restored.panelByNodeId.invoke_child.fieldsByTab.base ?? [];

    expect(baseFields.some((field) => field.key === "command")).toBe(false);
    expect(baseFields.find((field) => field.key === "workflowRef")?.value).toBe(
      "child-flow",
    );
    expect(baseFields.some((field) => field.key === "responseMode")).toBe(
      false,
    );
  });
});
