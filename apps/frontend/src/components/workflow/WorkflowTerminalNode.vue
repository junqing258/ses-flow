<script setup lang="ts">
import { computed } from "vue";
import { Handle, Position, type NodeProps } from "@vue-flow/core";

import { type WorkflowNodeData } from "@/features/workflow/model";
import { cn } from "@/lib/utils";

const props = defineProps<NodeProps<WorkflowNodeData>>();
const executionStatus = computed(() => props.data.executionStatus);
const terminalClass = computed(() =>
  cn(
    'relative flex h-[64px] w-[64px] items-center justify-center rounded-full border-4 text-[12px] font-semibold text-white shadow-[0_18px_42px_rgba(15,23,42,0.14)] transition-transform duration-200',
    executionStatus.value === "running"
      ? 'scale-[1.06] ring-4 ring-cyan-100'
      : executionStatus.value === "success"
        ? 'ring-4 ring-emerald-100'
        : executionStatus.value === "waiting"
          ? 'ring-4 ring-amber-100'
          : executionStatus.value === "failed"
            ? 'ring-4 ring-rose-100'
            : props.data.active || props.selected
              ? 'scale-[1.03]'
              : 'hover:scale-[1.01]',
  ),
);
const terminalStyle = computed(() => {
  if (executionStatus.value === "success") {
    return {
      backgroundColor: "#10B981",
      borderColor: "#10B981",
    };
  }

  if (executionStatus.value === "waiting") {
    return {
      backgroundColor: "#F59E0B",
      borderColor: "#F59E0B",
    };
  }

  if (executionStatus.value === "failed") {
    return {
      backgroundColor: "#F43F5E",
      borderColor: "#F43F5E",
    };
  }

  return {
    backgroundColor: props.data.accent,
    borderColor: props.data.accent,
  };
});
</script>

<template>
  <div
    :class="terminalClass"
    :style="terminalStyle"
  >
    <div
      v-if="executionStatus === 'running'"
      class="pointer-events-none absolute inset-[-6px] rounded-full border-2 border-cyan-300/70 animate-pulse"
    />

    <Handle
      v-if="props.data.kind !== 'start'"
      id="in"
      type="target"
      :position="Position.Left"
      class="!left-0 !h-3 !w-3 !-translate-x-1/2 !border-2 !border-[var(--terminal-accent)] !bg-white"
      :style="{ '--terminal-accent': props.data.accent }"
    />

    <span>{{ props.data.title }}</span>

    <Handle
      v-if="props.data.kind !== 'end'"
      id="out"
      type="source"
      :position="Position.Right"
      class="!right-0 !h-3 !w-3 !translate-x-1/2 !border-2 !border-[var(--terminal-accent)] !bg-white"
      :style="{ '--terminal-accent': props.data.accent }"
    />
  </div>
</template>
