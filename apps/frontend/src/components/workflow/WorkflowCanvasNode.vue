<script setup lang="ts">
import { computed } from "vue";
import { Handle, Position, type NodeProps } from "@vue-flow/core";

import { WORKFLOW_ICON_MAP, type WorkflowNodeData } from "@/features/workflow/model";
import { cn } from "@/lib/utils";

const props = defineProps<NodeProps<WorkflowNodeData>>();

const IconComponent = computed(() => WORKFLOW_ICON_MAP[props.data.icon]);
const isActive = computed(() => Boolean(props.data.active || props.selected));
const isSwitchNode = computed(() => props.data.kind === "switch");
</script>

<template>
  <div
    :class="
      cn(
        'group relative w-[220px] rounded-[18px] border-2 bg-white shadow-[0_18px_48px_rgba(15,23,42,0.08)] transition-all duration-200',
        isActive
          ? 'translate-y-[-2px] border-[var(--node-accent)] ring-4 ring-[color-mix(in_srgb,var(--node-accent)_16%,white)]'
          : 'border-[color-mix(in_srgb,var(--node-accent)_84%,white)] hover:translate-y-[-1px] hover:shadow-[0_22px_54px_rgba(15,23,42,0.12)]',
      )
    "
    :style="{ '--node-accent': data.accent }"
  >
    <Handle
      id="in"
      type="target"
      :position="Position.Top"
      class="!top-0 !h-3 !w-3 !-translate-y-1/2 !border-2 !border-[var(--node-accent)] !bg-white"
    />

    <div class="flex h-[78px] overflow-hidden rounded-[14px]">
      <div class="flex w-[56px] items-center justify-center bg-[var(--node-accent)] text-white">
        <component :is="IconComponent" class="h-5 w-5" />
      </div>

      <div class="flex min-w-0 flex-1 flex-col justify-center gap-1 px-4">
        <div class="flex items-center justify-between gap-2">
          <p class="truncate text-[12px] font-medium text-slate-500">{{ data.title }}</p>

          <span
            v-if="data.status === 'published'"
            class="inline-flex items-center gap-1 rounded-full bg-emerald-50 px-2 py-1 text-[10px] font-semibold text-emerald-700"
          >
            <span class="h-1.5 w-1.5 rounded-full bg-emerald-500" />
            已发布
          </span>
        </div>

        <p class="truncate text-[14px] font-semibold tracking-[0.01em] text-slate-900">{{ data.subtitle }}</p>
      </div>
    </div>

    <Handle
      v-if="!isSwitchNode"
      id="out"
      type="source"
      :position="Position.Bottom"
      class="!bottom-0 !h-3 !w-3 !translate-y-1/2 !border-2 !border-[var(--node-accent)] !bg-white"
    />

    <template v-else>
      <Handle
        id="branch-a"
        type="source"
        :position="Position.Left"
        class="!left-0 !h-3 !w-3 !-translate-x-1/2 !border-2 !border-[var(--node-accent)] !bg-white"
      />
      <Handle
        id="branch-b"
        type="source"
        :position="Position.Right"
        class="!right-0 !h-3 !w-3 !translate-x-1/2 !border-2 !border-[var(--node-accent)] !bg-white"
      />
    </template>
  </div>
</template>
