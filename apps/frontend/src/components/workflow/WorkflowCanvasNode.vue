<script setup lang="ts">
import { computed } from "vue";
import {
  Handle,
  Position,
  type NodeProps,
} from "@vue-flow/core";

import {
  type WorkflowBranchHandle,
  type WorkflowNodeData,
} from "@/features/workflow/model";
import { cn } from "@/lib/utils";
import WorkflowIcon from "./WorkflowIcon.vue";

const props = defineProps<NodeProps<WorkflowNodeData>>();

const isActive = computed(() => Boolean(props.data.active || props.selected));
const executionStatus = computed(() => props.data.executionStatus);
const isBranchNode = computed(
  () => props.data.kind === "switch" || props.data.kind === "if-else",
);
const branchHandles = computed<WorkflowBranchHandle[]>(() => {
  if (props.data.branchHandles?.length) {
    return props.data.branchHandles;
  }

  if (props.data.kind === "if-else") {
    return [
      { id: "branch-a", label: "then" },
      { id: "branch-b", isDefault: true, label: "else" },
    ];
  }

  return [];
});
const branchNodeHeight = computed(() =>
  isBranchNode.value ? Math.max(78, 48 + branchHandles.value.length * 26) : 78,
);
const resolveBranchTop = (index: number, total: number) => {
  if (total <= 1) {
    return "50%";
  }

  const start = 28;
  const end = 72;
  const step = (end - start) / (total - 1);

  return `${start + index * step}%`;
};
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
    'workflow-canvas-node group relative w-[220px] border-2 bg-white shadow-[0_18px_48px_rgba(15,23,42,0.08)] transition-all duration-200',
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
    :style="{ '--node-accent': data.accent, minHeight: `${branchNodeHeight}px` }"
  >
    <div
      v-if="executionStatus === 'running'"
      class="workflow-canvas-node__pulse pointer-events-none absolute -inset-1 animate-pulse bg-cyan-200/20"
    />

    <Handle
      id="in"
      type="target"
      :position="Position.Left"
      class="left-0! h-3! w-3! -translate-x-1/2! border-2! !border-(--node-accent)! bg-white!"
    />

    <div
      class="workflow-canvas-node__body flex overflow-hidden"
      :style="{ height: `${branchNodeHeight}px` }"
    >
      <div
        class="workflow-canvas-node__accent flex w-14 items-center justify-center bg-(--node-accent) text-white"
      >
        <WorkflowIcon
          :icon="data.icon"
          :alt="data.title"
          class="h-5 w-5"
        />
      </div>

      <div class="flex min-w-0 flex-1 flex-col justify-center gap-1 px-4">
        <div class="flex items-center justify-between gap-2">
          <p class="truncate text-[12px] font-medium text-slate-500">
            {{ data.title }}
          </p>
          <span
            v-if="executionStatus"
            class="inline-flex items-center gap-1 rounded-full px-2 py-1 text-[10px] font-semibold ring-1 z-2"
            :class="statusClass"
          >
            <span
              class="h-1.5 w-1.5 rounded-full "
              :class="executionStatus === 'running' ? 'animate-pulse bg-cyan-500' : 'bg-current opacity-70'"
            />
            <span class="truncate">{{ statusLabel }}</span>
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
      class="right-0! h-3! w-3! translate-x-1/2! border-2! !border-(--node-accent)! bg-white!"
    />

    <template v-else>
      <template
        v-for="(branch, index) in branchHandles"
        :key="branch.id"
      >
        <div
          class="pointer-events-none absolute z-10 rounded-full px-2 py-0.5 text-[10px] font-semibold whitespace-nowrap shadow-sm ring-1"
          :class="
            branch.isDefault
              ? 'bg-slate-900/90 text-white ring-slate-900/80'
              : 'bg-white text-slate-500 ring-slate-200'
          "
          :style="{
            top: resolveBranchTop(index, branchHandles.length),
            left: 'calc(100% + 14px)',
            transform: 'translateY(-50%)',
          }"
        >
          {{ branch.label }}
        </div>

        <Handle
          :id="branch.id"
          type="source"
          :position="Position.Right"
          connectable="single"
          class="right-0! h-3! w-3! translate-x-1/2! -translate-y-1/2! border-2! !border-(--node-accent)! bg-white!"
          :style="{
            top: resolveBranchTop(index, branchHandles.length),
          }"
        />
      </template>
    </template>
  </div>
</template>

<style scoped>
.workflow-canvas-node {
  --workflow-node-border-width: 2px;
  --workflow-node-radius: var(
    --el-dialog-border-radius,
    var(--el-border-radius-base)
  );
  --workflow-node-inner-radius: calc(
    var(--workflow-node-radius) - var(--workflow-node-border-width)
  );
  border-radius: var(--el-dialog-border-radius, var(--el-border-radius-base));
}

.workflow-canvas-node__pulse {
  border-radius: calc(
    var(--el-dialog-border-radius, var(--el-border-radius-base)) + 4px
  );
}

.workflow-canvas-node__body {
  border-radius: var(--workflow-node-inner-radius);
}

.workflow-canvas-node__accent {
  border-bottom-left-radius: var(--workflow-node-inner-radius);
  border-top-left-radius: var(--workflow-node-inner-radius);
}
</style>
