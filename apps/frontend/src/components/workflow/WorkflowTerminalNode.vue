<script setup lang="ts">
import { Handle, Position, type NodeProps } from "@vue-flow/core";

import { type WorkflowNodeData } from "@/features/workflow/model";
import { cn } from "@/lib/utils";

const props = defineProps<NodeProps<WorkflowNodeData>>();
</script>

<template>
  <div
    :class="
      cn(
        'relative flex h-[64px] w-[64px] items-center justify-center rounded-full border-4 text-[12px] font-semibold text-white shadow-[0_18px_42px_rgba(15,23,42,0.14)] transition-transform duration-200',
        props.data.active || props.selected ? 'scale-[1.03]' : 'hover:scale-[1.01]',
      )
    "
    :style="{ backgroundColor: props.data.accent, borderColor: props.data.accent }"
  >
    <Handle
      v-if="props.data.kind !== 'start'"
      id="in"
      type="target"
      :position="Position.Top"
      class="!top-0 !h-3 !w-3 !-translate-y-1/2 !border-2 !border-[var(--terminal-accent)] !bg-white"
      :style="{ '--terminal-accent': props.data.accent }"
    />

    <span>{{ props.data.title }}</span>

    <Handle
      v-if="props.data.kind !== 'end'"
      id="out"
      type="source"
      :position="Position.Bottom"
      class="!bottom-0 !h-3 !w-3 !translate-y-1/2 !border-2 !border-[var(--terminal-accent)] !bg-white"
      :style="{ '--terminal-accent': props.data.accent }"
    />
  </div>
</template>
