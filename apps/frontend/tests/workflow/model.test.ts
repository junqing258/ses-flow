import { describe, expect, it } from "vitest";

import {
  WORKFLOW_PALETTE_CATEGORIES,
  createWorkflowNodeDraft,
} from "@/features/workflow/model";

describe("createWorkflowNodeDraft", () => {
  it("uses a stable non-empty fallback id for palette items with non-latin labels", () => {
    const taskPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap((category) => category.items).find(
      (item) => item.id === "palette-task",
    );

    expect(taskPaletteItem).toBeDefined();

    const { node } = createWorkflowNodeDraft(taskPaletteItem!, { x: 120, y: 240 }, []);

    expect(node.id).toBe("task");
    expect(node.data.nodeKey).toBe("task");
  });
});
