<script setup lang="ts">
import { computed } from "vue";
import {
  Handle,
  Position,
  type HandleConnectableFunc,
  type NodeProps,
} from "@vue-flow/core";

import {
  WORKFLOW_ICON_MAP,
  type WorkflowNodeData,
} from "@/features/workflow/model";
import { cn } from "@/lib/utils";

const props = defineProps<NodeProps<WorkflowNodeData>>();

const IconComponent = computed(() => WORKFLOW_ICON_MAP[props.data.icon]);
const isActive = computed(() => Boolean(props.data.active || props.selected));
const executionStatus = computed(() => props.data.executionStatus);
const isBranchNode = computed(
  () => props.data.kind === "switch" || props.data.kind === "if-else",
);
const singleConnectionHandle: HandleConnectableFunc = (_node, connectedEdges) =>
  connectedEdges.length < 1;
const statusLabel = computed(() => {
  switch (executionStatus.value) {
    case "running":
      return "运行中";
    case "success":
      return "成功";
    case "waiting":
      return "等待";
    case "failed":
      return "失败";
    case "skipped":
      return "跳过";
    default:
      return "";
  }
});
const statusClass = computed(() => {
  switch (executionStatus.value) {
    case "running":
      return "bg-cyan-50 text-cyan-700 ring-cyan-200";
    case "success":
      return "bg-emerald-50 text-emerald-700 ring-emerald-200";
    case "waiting":
      return "bg-amber-50 text-amber-700 ring-amber-200";
    case "failed":
      return "bg-rose-50 text-rose-700 ring-rose-200";
    case "skipped":
      return "bg-slate-100 text-slate-500 ring-slate-200";
    default:
      return "";
  }
});
const containerClass = computed(() =>
  cn(
    'group relative w-[220px] rounded-[18px] border-2 bg-white shadow-[0_18px_48px_rgba(15,23,42,0.08)] transition-all duration-200',
    executionStatus.value === "running"
      ? 'translate-y-[-2px] border-[var(--node-accent)] ring-4 ring-cyan-100 shadow-[0_22px_54px_rgba(34,211,238,0.18)]'
      : executionStatus.value === "failed"
        ? 'border-rose-300 ring-4 ring-rose-100'
        : executionStatus.value === "waiting"
          ? 'border-amber-300 ring-4 ring-amber-100'
          : executionStatus.value === "success"
            ? 'border-emerald-300 ring-4 ring-emerald-100'
            : isActive.value
              ? 'translate-y-[-2px] border-[var(--node-accent)] ring-4 ring-[color-mix(in_srgb,var(--node-accent)_16%,white)]'
              : 'border-[color-mix(in_srgb,var(--node-accent)_84%,white)] hover:translate-y-[-1px] hover:shadow-[0_22px_54px_rgba(15,23,42,0.12)]',
  ),
);
</script>

<template>
  <div
    :class="containerClass"
    :style="{ '--node-accent': data.accent }"
  >
    <div
      v-if="executionStatus === 'running'"
      class="pointer-events-none absolute -inset-1 rounded-[22px] animate-pulse bg-cyan-200/20"
    />

    <Handle
      id="in"
      type="target"
      :position="Position.Left"
      class="!left-0 !h-3 !w-3 !-translate-x-1/2 !border-2 !border-(--node-accent)! !bg-white"
    />

    <div class="flex h-[78px] overflow-hidden rounded-[14px]">
      <div
        class="flex w-[56px] items-center justify-center bg-[var(--node-accent)] text-white"
      >
        <component :is="IconComponent" class="h-5 w-5" />
      </div>

      <div class="flex min-w-0 flex-1 flex-col justify-center gap-1 px-4">
        <div class="flex items-center justify-between gap-2">
          <p class="truncate text-[12px] font-medium text-slate-500">
            {{ data.title }}
          </p>
          <span
            v-if="executionStatus"
            class="inline-flex items-center gap-1 rounded-full px-2 py-1 text-[10px] font-semibold ring-1"
            :class="statusClass"
          >
            <span
              class="h-1.5 w-1.5 rounded-full"
              :class="executionStatus === 'running' ? 'animate-pulse bg-cyan-500' : 'bg-current opacity-70'"
            />
            {{ statusLabel }}
          </span>
        </div>

        <p
          class="truncate text-[14px] font-semibold tracking-[0.01em] text-slate-900"
        >
          {{ data.subtitle }}
        </p>
      </div>
    </div>

    <Handle
      v-if="!isBranchNode"
      id="out"
      type="source"
      :position="Position.Right"
      class="!right-0 !h-3 !w-3 !translate-x-1/2 !border-2 !border-(--node-accent)! !bg-white"
    />

    <template v-else>
      <Handle
        id="branch-a"
        type="source"
        :position="Position.Right"
        :connectable="singleConnectionHandle"
        class="!right-0 !top-[32%] !h-3 !w-3 !translate-x-1/2 !-translate-y-1/2 !border-2 !border-(--node-accent)! !bg-white"
      />
      <Handle
        id="branch-b"
        type="source"
        :position="Position.Right"
        :connectable="singleConnectionHandle"
        class="!right-0 !top-[68%] !h-3 !w-3 !translate-x-1/2 !-translate-y-1/2 !border-2 !border-(--node-accent)! !bg-white"
      />
    </template>
  </div>
</template>
