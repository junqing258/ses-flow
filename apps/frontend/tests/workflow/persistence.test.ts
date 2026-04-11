import { describe, expect, it } from "vitest";

import {
  createNewWorkflowEditorState,
  createPersistedWorkflowDocument,
  createWorkflowEditorStateFromDocument,
} from "@/features/workflow/persistence";

describe("workflow persistence", () => {
  it("persists run mode and run draft fields", () => {
    const state = createNewWorkflowEditorState();
    const runDraft = {
      body: '{\n  "orderId": "SO-20002"\n}',
      env: '{\n  "tenantId": "tenant-b",\n  "warehouseId": "WHS-HZ-01"\n}',
      headers: '{\n  "x-request-id": "wf-run-002"\n}',
      triggerMode: "webhook" as const,
    };

    const document = createPersistedWorkflowDocument(state.nodes, state.edges, state.panelByNodeId, {
      activeTab: state.activeTab,
      pageMode: "run",
      runDraft,
      selectedNodeId: state.selectedNodeId,
      status: "draft",
      version: "v3",
      workflowId: "sorting-main-flow",
      workflowName: "sorting-main-flow",
    });

    expect(document.editor.pageMode).toBe("run");
    expect(document.editor.runDraft).toEqual(runDraft);

    const restored = createWorkflowEditorStateFromDocument(document);

    expect(restored.pageMode).toBe("run");
    expect(restored.runDraft).toEqual(runDraft);
  });

  it("falls back to default run state for older documents", () => {
    const state = createNewWorkflowEditorState();
    const document = createPersistedWorkflowDocument(state.nodes, state.edges, state.panelByNodeId, {
      activeTab: state.activeTab,
      selectedNodeId: state.selectedNodeId,
      status: "draft",
      version: "v3",
      workflowId: "sorting-main-flow",
      workflowName: "sorting-main-flow",
    });

    delete document.editor.pageMode;
    delete document.editor.runDraft;

    const restored = createWorkflowEditorStateFromDocument(document);

    expect(restored.pageMode).toBe("edit");
    expect(restored.runDraft.triggerMode).toBe("manual");
    expect(restored.runDraft.body).toContain('"orderId": "SO-10001"');
    expect(restored.runDraft.env).toContain('"tenantId": "tenant-a"');
    expect(restored.runDraft.env).toContain('"warehouseId": "WHS-SH-01"');
  });

  it("does not persist runtime execution status into workflow documents", () => {
    const state = createNewWorkflowEditorState();

    state.nodes[0]!.data.executionStatus = "success";

    const document = createPersistedWorkflowDocument(state.nodes, state.edges, state.panelByNodeId, {
      activeTab: state.activeTab,
      selectedNodeId: state.selectedNodeId,
      status: "draft",
      version: "v3",
      workflowId: "sorting-main-flow",
      workflowName: "sorting-main-flow",
    });

    expect(document.graph.nodes[0]?.data.executionStatus).toBeUndefined();
  });

  it("clears historical execution status when restoring older documents", () => {
    const state = createNewWorkflowEditorState();
    const document = createPersistedWorkflowDocument(state.nodes, state.edges, state.panelByNodeId, {
      activeTab: state.activeTab,
      selectedNodeId: state.selectedNodeId,
      status: "draft",
      version: "v3",
      workflowId: "sorting-main-flow",
      workflowName: "sorting-main-flow",
    });

    document.graph.nodes[0]!.data.executionStatus = "success";

    const restored = createWorkflowEditorStateFromDocument(document);

    expect(restored.nodes[0]?.data.executionStatus).toBeUndefined();
  });
});
