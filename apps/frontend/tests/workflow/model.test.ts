import { describe, expect, it } from "vitest";

import {
  WORKFLOW_PALETTE_CATEGORIES,
  createWorkflowNodeDraft,
  getWorkflowFieldSelectOptions,
  resolveWorkflowReferenceId,
} from "@/features/workflow/model";

describe("createWorkflowNodeDraft", () => {
  it("uses the palette id as a stable fallback when the label is non-latin", () => {
    const { node } = createWorkflowNodeDraft(
      {
        id: "palette-review-step",
        kind: "effect",
        label: "人工复核",
        icon: "activity",
        accent: "#8B5CF6",
      },
      { x: 120, y: 240 },
      [],
    );

    expect(node.id).toBe("review_step");
    expect(node.data.nodeKey).toBe("review_step");
  });

  it("provides editable options for fetch request methods", () => {
    const fetchPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-fetch");

    expect(fetchPaletteItem).toBeDefined();

    const { panel } = createWorkflowNodeDraft(
      fetchPaletteItem!,
      { x: 160, y: 240 },
      [],
    );
    const methodField = panel.fieldsByTab.base?.find(
      (field) => field.key === "method",
    );

    expect(methodField).toBeDefined();
    expect(getWorkflowFieldSelectOptions(panel, methodField!)).toEqual([
      { label: "GET", value: "GET" },
      { label: "POST", value: "POST" },
    ]);
  });

  it("creates set-state nodes with a writable state path and value", () => {
    const setStatePaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-set-state");

    expect(setStatePaletteItem).toBeDefined();

    const { node, panel } = createWorkflowNodeDraft(
      setStatePaletteItem!,
      { x: 180, y: 240 },
      [],
    );
    const statePathField = panel.fieldsByTab.base?.find(
      (field) => field.key === "statePath",
    );
    const valueField = panel.fieldsByTab.mapping?.find(
      (field) => field.key === "value",
    );

    expect(node.data.kind).toBe("set-state");
    expect(node.data.title).toBe("Set State");
    expect(statePathField?.value).toBe("statePatch");
    expect(valueField?.value).toContain("handledBy");
  });

  it("maps switch fallback select options from current branches", () => {
    const switchPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-switch");

    expect(switchPaletteItem).toBeDefined();

    const { panel } = createWorkflowNodeDraft(
      switchPaletteItem!,
      { x: 220, y: 240 },
      [],
    );
    const fallbackField = panel.fieldsByTab.base?.find(
      (field) => field.key === "fallback",
    );

    expect(fallbackField).toBeDefined();
    expect(getWorkflowFieldSelectOptions(panel, fallbackField!)).toEqual([
      { label: "A", value: "branch-a" },
      { label: "B", value: "branch-b" },
    ]);
  });

  it("creates if-else nodes with default then and else branches", () => {
    const ifElsePaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-if-else");

    expect(ifElsePaletteItem).toBeDefined();

    const { node, panel } = createWorkflowNodeDraft(
      ifElsePaletteItem!,
      { x: 220, y: 240 },
      [],
    );
    const expressionField = panel.fieldsByTab.base?.find(
      (field) => field.key === "expression",
    );
    const fallbackField = panel.fieldsByTab.base?.find(
      (field) => field.key === "fallback",
    );

    expect(node.data.kind).toBe("if-else");
    expect(node.data.title).toBe("If / Else");
    expect(node.data.branchHandles).toEqual([
      { id: "branch-a", label: "then" },
      { id: "branch-b", label: "else", isDefault: true },
    ]);
    expect(expressionField?.value).toBe("payload.condition === true");
    expect(fallbackField?.value).toBe("else");
  });

  it("creates dedicated workflow selection fields for sub-workflow nodes", () => {
    const subWorkflowPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-subflow");

    expect(subWorkflowPaletteItem).toBeDefined();

    const { node, panel } = createWorkflowNodeDraft(
      subWorkflowPaletteItem!,
      { x: 260, y: 240 },
      [],
    );
    const workflowRefField = panel.fieldsByTab.base?.find(
      (field) => field.key === "workflowRef",
    );

    expect(node.data.kind).toBe("sub-workflow");
    expect(node.data.title).toBe("Sub-Workflow");
    expect(workflowRefField).toBeDefined();
    expect(workflowRefField?.type).toBe("select");
    expect(
      panel.fieldsByTab.mapping?.find((field) => field.key === "payload")
        ?.value,
    ).toBe("{{input}}");
  });

  it("resolves sub-workflow references to workflow ids", () => {
    expect(
      resolveWorkflowReferenceId("child-flow", [
        {
          workflowId: "wf-child-1",
          workflowKey: "child-flow",
        },
      ]),
    ).toBe("wf-child-1");
  });

  it("prefers an exact workflow id match over a workflow key match", () => {
    expect(
      resolveWorkflowReferenceId("child-flow", [
        {
          workflowId: "wf-child-1",
          workflowKey: "child-flow",
        },
        {
          workflowId: "child-flow",
          workflowKey: "legacy-child-flow",
        },
      ]),
    ).toBe("child-flow");
  });
});
