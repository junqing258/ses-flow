import { describe, expect, it } from "vitest";

import {
  WORKFLOW_PALETTE_CATEGORIES,
  createWorkflowNodeDraft,
  getWorkflowFieldSelectOptions,
} from "@/features/workflow/model";

describe("createWorkflowNodeDraft", () => {
  it("uses the palette id as a stable fallback when the label is non-latin", () => {
    const taskPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-task");

    expect(taskPaletteItem).toBeDefined();

    const { node } = createWorkflowNodeDraft(
      taskPaletteItem!,
      { x: 120, y: 240 },
      [],
    );

    expect(node.id).toBe("task");
    expect(node.data.nodeKey).toBe("task");
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
  });
});
