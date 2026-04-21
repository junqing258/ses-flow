<template>
  <div class="space-y-4">
    <div
      class="rounded-[18px] border border-[var(--panel-border)]/80 bg-white p-4"
    >
      <div class="flex items-center justify-between gap-3">
        <div>
          <p class="text-xs font-semibold tracking-wide text-[var(--app-muted)]">
            执行时间线
          </p>
          <p class="mt-1 text-[11px] text-[#7a7f86]">
            {{ timeline.length ? `${timeline.length} steps` : emptyText }}
          </p>
        </div>
      </div>

      <div v-if="timeline.length" class="mt-4 space-y-3">
        <article
          v-for="(item, index) in timeline"
          :key="`${item.nodeId}-${index}`"
          class="rounded-2xl border p-4 transition"
          :class="cardClass(item)"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0 flex-1">
              <p class="truncate text-[13px] font-semibold text-[var(--text)]">
                {{ nodeTitle(item.nodeId) }}
              </p>
              <p class="mt-1 text-[11px] text-[#7a7f86]">
                {{ item.nodeType }} · {{ item.nodeId }}
              </p>
            </div>
            <span
              class="rounded-full px-2 py-0.5 text-[11px] font-semibold"
              :class="statusClass(item.status)"
            >
              {{ item.status }}
            </span>
          </div>

          <div v-if="item.durationMs !== undefined" class="mt-3">
            <div class="mb-1 flex items-center justify-between text-[11px] text-[var(--app-muted)]">
              <span>耗时</span>
              <span>{{ formatDuration(item.durationMs) }}</span>
            </div>
            <div class="h-2 rounded-full bg-white/90 ring-1 ring-[var(--panel-border)]/70">
              <div
                class="h-full rounded-full bg-[linear-gradient(90deg,#1d4ed8,#22c55e)] transition-[width] duration-300"
                :style="{ width: `${durationPercent(item.durationMs)}%` }"
              />
            </div>
          </div>

          <div class="mt-3 grid gap-2 text-[11px] text-[var(--app-muted)] sm:grid-cols-2">
            <div class="rounded-xl bg-white/80 px-3 py-2 ring-1 ring-[var(--panel-border)]/70">
              <span class="font-semibold text-[#354a56]">开始</span>
              <p class="mt-1 break-all">{{ formatDateTime(item.startedAt) }}</p>
            </div>
            <div class="rounded-xl bg-white/80 px-3 py-2 ring-1 ring-[var(--panel-border)]/70">
              <span class="font-semibold text-[#354a56]">结束</span>
              <p class="mt-1 break-all">{{ formatDateTime(item.endedAt) }}</p>
            </div>
          </div>

          <div
            v-if="item.inputSummary || item.outputSummary"
            class="mt-3 grid gap-2 text-[11px] text-[var(--app-muted)]"
          >
            <div
              v-if="item.inputSummary"
              class="rounded-xl bg-white/80 px-3 py-2 ring-1 ring-[var(--panel-border)]/70"
            >
              <span class="font-semibold text-[#354a56]">Input</span>
              <p class="mt-1 whitespace-pre-wrap break-words">
                {{ item.inputSummary }}
              </p>
            </div>
            <div
              v-if="item.outputSummary"
              class="rounded-xl bg-white/80 px-3 py-2 ring-1 ring-[var(--panel-border)]/70"
            >
              <span class="font-semibold text-[#354a56]">Output</span>
              <p class="mt-1 whitespace-pre-wrap break-words">
                {{ item.outputSummary }}
              </p>
            </div>
          </div>

          <div
            v-if="item.errorCode || item.errorDetail || templateSteps(item).length"
            class="mt-3 rounded-xl border border-rose-200 bg-rose-50/90 px-3 py-3 text-[11px] text-rose-700"
          >
            <p v-if="item.errorCode" class="font-semibold">
              {{ item.errorCode }}
            </p>
            <p v-if="item.errorDetail" class="mt-1 whitespace-pre-wrap break-words">
              {{ item.errorDetail }}
            </p>
            <p
              v-if="item.recoveryHint"
              class="mt-2 rounded-lg bg-white/80 px-2.5 py-2 text-rose-800 ring-1 ring-rose-100"
            >
              建议：{{ item.recoveryHint }}
            </p>
            <div v-if="templateSteps(item).length" class="mt-2 space-y-1">
              <p class="font-semibold text-rose-800">排查模板</p>
              <p
                v-for="(step, stepIndex) in templateSteps(item)"
                :key="`${item.nodeId}-step-${stepIndex}`"
              >
                {{ stepIndex + 1 }}. {{ step }}
              </p>
            </div>
          </div>

          <div v-if="item.logs?.length" class="mt-3 space-y-1">
            <p
              v-for="(log, logIndex) in item.logs"
              :key="`${item.nodeId}-log-${logIndex}`"
              class="rounded-xl bg-white px-2.5 py-2 font-mono text-[11px] leading-5 text-[var(--app-muted)] ring-1 ring-[var(--panel-border)]/80"
            >
              [{{ log.level }}] {{ log.message }}
            </p>
          </div>
        </article>
      </div>

      <div
        v-else
        class="mt-4 flex min-h-40 items-center justify-center rounded-2xl border border-dashed border-[var(--panel-border)] bg-[var(--panel-soft)]/80 px-6 text-center text-xs leading-5 text-[#7a7f86]"
      >
        {{ emptyText }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

import type { WorkflowRunSummary, WorkflowRunTimelineItem } from "@/features/workflow/runner";
import { getTroubleshootTemplateSteps } from "@/features/workflow/troubleshoot-templates";

const props = withDefaults(
  defineProps<{
    emptyText?: string;
    nodeNameMap?: Record<string, string>;
    summary: WorkflowRunSummary | null;
    workflowKey?: string;
  }>(),
  {
    emptyText: "运行后会按顺序展示每个节点的执行结果和日志。",
    nodeNameMap: () => ({}),
    workflowKey: "",
  },
);

const timeline = computed(() => props.summary?.timeline ?? []);
const maxDurationMs = computed(() =>
  Math.max(
    1,
    ...timeline.value.map((item) => Math.max(item.durationMs ?? 0, 0)),
  ),
);

const nodeTitle = (nodeId: string) => props.nodeNameMap[nodeId] ?? nodeId;

const durationPercent = (durationMs?: number) =>
  Math.max(8, Math.min(100, ((durationMs ?? 0) / maxDurationMs.value) * 100));

const formatDuration = (durationMs?: number) => {
  if (durationMs === undefined) {
    return "--";
  }

  return `${Math.max(0, Math.round(durationMs))} ms`;
};

const formatDateTime = (value?: string) => {
  if (!value) {
    return "--";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString();
};

const statusClass = (status: string) => {
  switch (status) {
    case "running":
      return "bg-cyan-50 text-cyan-700";
    case "waiting":
      return "bg-amber-50 text-amber-700";
    case "failed":
      return "bg-rose-50 text-rose-700";
    case "success":
    case "completed":
      return "bg-emerald-50 text-emerald-700";
    default:
      return "bg-slate-100 text-slate-600";
  }
};

const cardClass = (item: WorkflowRunTimelineItem) => {
  if (item.status === "failed") {
    return "border-rose-200 bg-rose-50/45";
  }

  if (item.status === "waiting") {
    return "border-amber-200 bg-amber-50/50";
  }

  return "border-[var(--panel-border)]/80 bg-[var(--panel-soft)]/90";
};

const templateSteps = (item: WorkflowRunTimelineItem) =>
  getTroubleshootTemplateSteps(
    props.workflowKey || props.summary?.workflowKey || "",
    item.errorCode,
  );
</script>
