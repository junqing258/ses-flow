import { describe, expect, it } from "vitest";

import {
  WORKFLOW_PALETTE_CATEGORIES,
  createWorkflowNodeDraft,
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
});
