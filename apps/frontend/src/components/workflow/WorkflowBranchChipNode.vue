<script setup lang="ts">
import type { NodeProps } from "@vue-flow/core";

import { type WorkflowNodeData } from "@/features/workflow/model";

defineProps<NodeProps<WorkflowNodeData>>();
</script>

<template>
  <div class="workflow-branch-chip-node border border-slate-200 bg-white/88 px-3 py-1 text-[11px] font-medium text-slate-500 shadow-sm">
    {{ data.title }}
  </div>
</template>

<style scoped>
.workflow-branch-chip-node {
  border-radius: var(--el-dialog-border-radius, var(--el-border-radius-base));
}
</style>
