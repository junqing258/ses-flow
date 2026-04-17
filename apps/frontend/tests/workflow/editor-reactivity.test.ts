import { computed, ref, shallowRef, triggerRef } from "vue";
import { describe, expect, it } from "vitest";

import {
  WORKFLOW_PALETTE_CATEGORIES,
  createWorkflowNodeDraft,
  getSwitchBranches,
  setSwitchBranches,
} from "@/features/workflow/model";

describe("workflow editor switch branch reactivity", () => {
  it("refreshes selected switch branches after adding a branch", () => {
    const switchPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-switch");

    expect(switchPaletteItem).toBeDefined();

    const { panel } = createWorkflowNodeDraft(
      switchPaletteItem!,
      { x: 220, y: 240 },
      [],
    );
    const panelByNodeId = shallowRef({
      switch_biz_type: panel,
    });
    const selectedNodeId = ref("switch_biz_type");
    const selectedSwitchBranches = computed(() =>
      getSwitchBranches(panelByNodeId.value[selectedNodeId.value]),
    );

    expect(selectedSwitchBranches.value.map((branch) => branch.label)).toEqual([
      "A",
      "B",
    ]);

    setSwitchBranches(panel, [
      ...getSwitchBranches(panel),
      { id: "branch-c", label: "C" },
    ]);
    triggerRef(panelByNodeId);

    expect(selectedSwitchBranches.value.map((branch) => branch.label)).toEqual([
      "A",
      "B",
      "C",
    ]);
  });
});
