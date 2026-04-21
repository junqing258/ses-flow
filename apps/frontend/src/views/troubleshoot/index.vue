<template>
  <section class="min-h-screen bg-slate-50 text-slate-900">
    <div class="mx-auto max-w-7xl px-4 py-6 sm:px-8 lg:px-10">
      <header class="flex flex-wrap items-center justify-between gap-4">
        <div>
          <p
            class="text-[11px] font-semibold uppercase tracking-[0.24em] text-cyan-700"
          >
            Troubleshoot
          </p>
          <h2 class="mt-2 text-3xl font-semibold tracking-tight text-slate-950">
            运行排障工作台
          </h2>
          <!-- <p class="mt-2 max-w-2xl text-sm leading-6 text-slate-500">
            按 `runId`、`requestId`、订单号或波次号定位运行记录，直接查看
            timeline、错误提示和状态快照。
          </p> -->
        </div>
      </header>

      <section
        class="mt-6 rounded-2xl border border-white/70 bg-white/90 p-4 backdrop-blur"
      >
        <div class="grid gap-2.5 md:grid-cols-2 xl:grid-cols-5">
          <Input
            v-model="filters.runId"
            class="h-10 rounded-xl border-slate-200 bg-slate-50 px-3.5 text-sm"
            placeholder="Run ID"
          />
          <Input
            v-model="filters.requestId"
            class="h-10 rounded-xl border-slate-200 bg-slate-50 px-3.5 text-sm"
            placeholder="Request ID"
          />
          <Input
            v-model="filters.orderNo"
            class="h-10 rounded-xl border-slate-200 bg-slate-50 px-3.5 text-sm"
            placeholder="订单号"
          />
          <Input
            v-model="filters.waveNo"
            class="h-10 rounded-xl border-slate-200 bg-slate-50 px-3.5 text-sm"
            placeholder="波次号"
          />
          <Button
            class="h-10 rounded-xl bg-slate-950 px-4 text-sm text-white hover:bg-slate-800"
            :disabled="isSearching"
            @click="handleSearch"
          >
            {{ isSearching ? "搜索中..." : "搜索" }}
          </Button>
        </div>
        <p v-if="searchError" class="mt-3 text-sm text-rose-600">
          {{ searchError }}
        </p>
      </section>

      <div class="mt-6 space-y-6">
        <section class="rounded-2xl border border-white/70 bg-white/92 p-4">
          <div class="flex items-center justify-between gap-3 px-2 pb-3">
            <div>
              <p class="text-xs font-semibold tracking-wide text-slate-500">
                搜索结果
              </p>
              <p class="mt-1 text-sm text-slate-400">{{ total }} 条记录</p>
            </div>
          </div>

          <div
            v-if="results.length === 0"
            class="flex min-h-28 items-center justify-center rounded-[22px] border border-dashed border-slate-200 bg-slate-50/80 px-6 text-center text-sm leading-6 text-slate-500"
          >
            输入检索条件后即可定位运行记录。
          </div>

          <div v-else class="flex gap-2.5 overflow-x-auto px-1 pb-1">
            <button
              v-for="item in results"
              :key="item.runId"
              type="button"
              class="min-w-[220px] max-w-[250px] shrink-0 rounded-[18px] border px-3.5 py-2.5 text-left transition"
              :class="
                item.runId === selectedRunId
                  ? 'border-cyan-200 bg-cyan-50/90'
                  : 'border-slate-200 bg-slate-50/70 hover:border-cyan-100 hover:bg-white'
              "
              @click="handleSelectRun(item.runId)"
            >
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <p class="truncate text-[13px] font-semibold text-slate-900">
                    {{ item.runId }}
                  </p>
                  <p class="mt-0.5 truncate text-[11px] text-slate-500">
                    {{ item.workflowKey }}
                  </p>
                </div>
                <span
                  class="rounded-full px-2 py-0.5 text-[11px] font-semibold"
                  :class="statusClass(item.status)"
                >
                  {{ item.status }}
                </span>
              </div>

              <div class="mt-2 grid gap-0.5 text-[11px] leading-5 text-slate-500">
                <p v-if="item.requestId" class="truncate">
                  Request: {{ item.requestId }}
                </p>
                <p v-if="item.orderNo" class="truncate">
                  订单: {{ item.orderNo }}
                </p>
                <p v-if="item.waveNo" class="truncate">
                  波次: {{ item.waveNo }}
                </p>
                <p class="truncate">
                  开始: {{ formatDateTime(item.startedAt) }}
                </p>
                <p v-if="item.durationMs !== undefined">
                  总耗时: {{ item.durationMs }} ms
                </p>
              </div>
            </button>
          </div>
        </section>

        <main class="space-y-6">
          <section class="rounded-2xl border border-white/70 bg-white/92 p-5">
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p class="text-xs font-semibold tracking-wide text-slate-500">
                  日志列表
                </p>
                <p class="mt-1 text-sm text-slate-400">
                  {{
                    selectedSummary
                      ? `共 ${selectedSummary.timeline.length} 条节点执行日志，点击查看详情`
                      : "先从上方搜索结果中选择一条运行记录"
                  }}
                </p>
              </div>
              <Button
                variant="outline"
                class="rounded-full border-slate-200 bg-white px-4 text-slate-700 hover:border-cyan-200 hover:bg-cyan-50 hover:text-cyan-700"
                :disabled="!selectedRunId || isLoadingSummary"
                @click="refreshSelectedRun"
              >
                {{ isLoadingSummary ? "刷新中..." : "刷新详情" }}
              </Button>
            </div>

            <p v-if="summaryError" class="mt-3 text-sm text-rose-600">
              {{ summaryError }}
            </p>

            <div
              v-if="!selectedSummary"
              class="mt-4 flex min-h-60 items-center justify-center rounded-[22px] border border-dashed border-slate-200 bg-slate-50/80 px-6 text-center text-sm leading-6 text-slate-500"
            >
              搜索后点击一条运行记录，这里会展示节点日志列表和详细内容。
            </div>

            <template v-else>
              <div class="mt-4 grid gap-3 md:grid-cols-4">
                <div class="rounded-2xl bg-slate-50 px-4 py-3">
                  <p
                    class="text-[11px] font-semibold tracking-wide text-slate-500"
                  >
                    状态
                  </p>
                  <p class="mt-2 text-base font-semibold text-slate-900">
                    {{ selectedSummary.status }}
                  </p>
                </div>
                <div class="rounded-2xl bg-slate-50 px-4 py-3">
                  <p
                    class="text-[11px] font-semibold tracking-wide text-slate-500"
                  >
                    当前节点
                  </p>
                  <p class="mt-2 text-base font-semibold text-slate-900">
                    {{ selectedSummary.currentNodeId ?? "--" }}
                  </p>
                </div>
                <div class="rounded-2xl bg-slate-50 px-4 py-3">
                  <p
                    class="text-[11px] font-semibold tracking-wide text-slate-500"
                  >
                    Timeline Steps
                  </p>
                  <p class="mt-2 text-base font-semibold text-slate-900">
                    {{ selectedSummary.timeline.length }}
                  </p>
                </div>
                <div class="rounded-2xl bg-slate-50 px-4 py-3">
                  <p
                    class="text-[11px] font-semibold tracking-wide text-slate-500"
                  >
                    最近输出
                  </p>
                  <p
                    class="mt-2 truncate text-base font-semibold text-slate-900"
                  >
                    {{ lastOutputSummary }}
                  </p>
                </div>
              </div>

              <div class="mt-5 grid gap-4 xl:grid-cols-[360px_minmax(0,1fr)]">
                <div
                  class="rounded-[22px] border border-slate-200 bg-slate-50/80 p-3"
                >
                  <div
                    class="flex items-center justify-between gap-3 px-2 pb-3"
                  >
                    <p
                      class="text-xs font-semibold tracking-wide text-slate-500"
                    >
                      节点日志
                    </p>
                    <span class="text-[11px] text-slate-400">
                      {{ selectedSummary.timeline.length }} 条
                    </span>
                  </div>

                  <div class="space-y-2">
                    <button
                      v-for="(item, index) in selectedSummary.timeline"
                      :key="`${item.nodeId}-${index}`"
                      type="button"
                      class="w-full rounded-[18px] border px-3 py-3 text-left transition"
                      :class="
                        index === selectedTimelineIndex
                          ? 'border-cyan-200 bg-cyan-50'
                          : 'border-slate-200 bg-white hover:border-cyan-100 hover:bg-cyan-50/40'
                      "
                      @click="handleSelectTimelineItem(index)"
                    >
                      <div class="flex items-start justify-between gap-2">
                        <div class="min-w-0">
                          <p
                            class="truncate text-sm font-semibold text-slate-900"
                          >
                            {{ item.nodeId }}
                          </p>
                          <p class="mt-1 text-[11px] text-slate-500">
                            {{ item.nodeType }}
                          </p>
                        </div>
                        <span
                          class="rounded-full px-2 py-0.5 text-[11px] font-semibold"
                          :class="statusClass(item.status)"
                        >
                          {{ item.status }}
                        </span>
                      </div>

                      <div class="mt-2 space-y-1 text-[11px] text-slate-500">
                        <p v-if="item.startedAt">
                          开始：{{ formatDateTime(item.startedAt) }}
                        </p>
                        <p v-if="item.durationMs !== undefined">
                          耗时：{{ item.durationMs }} ms
                        </p>
                        <p v-if="item.errorCode" class="text-rose-600">
                          {{ item.errorCode }}
                        </p>
                        <p v-else-if="item.outputSummary" class="truncate">
                          {{ item.outputSummary }}
                        </p>
                      </div>
                    </button>
                  </div>
                </div>

                <div
                  class="rounded-[22px] border border-slate-200 bg-white p-4"
                >
                  <div v-if="selectedTimelineItem" class="space-y-4">
                    <div
                      class="flex flex-wrap items-start justify-between gap-3"
                    >
                      <div>
                        <p
                          class="text-xs font-semibold tracking-wide text-slate-500"
                        >
                          节点详情
                        </p>
                        <p class="mt-2 text-lg font-semibold text-slate-950">
                          {{ selectedTimelineItem.nodeId }}
                        </p>
                        <p class="mt-1 text-sm text-slate-500">
                          {{ selectedTimelineItem.nodeType }}
                        </p>
                      </div>
                      <span
                        class="rounded-full px-3 py-1 text-xs font-semibold"
                        :class="statusClass(selectedTimelineItem.status)"
                      >
                        {{ selectedTimelineItem.status }}
                      </span>
                    </div>

                    <div class="grid gap-3 md:grid-cols-3">
                      <div class="rounded-2xl bg-slate-50 px-4 py-3">
                        <p
                          class="text-[11px] font-semibold tracking-wide text-slate-500"
                        >
                          开始时间
                        </p>
                        <p class="mt-2 text-sm font-medium text-slate-900">
                          {{ formatDateTime(selectedTimelineItem.startedAt) }}
                        </p>
                      </div>
                      <div class="rounded-2xl bg-slate-50 px-4 py-3">
                        <p
                          class="text-[11px] font-semibold tracking-wide text-slate-500"
                        >
                          结束时间
                        </p>
                        <p class="mt-2 text-sm font-medium text-slate-900">
                          {{ formatDateTime(selectedTimelineItem.endedAt) }}
                        </p>
                      </div>
                      <div class="rounded-2xl bg-slate-50 px-4 py-3">
                        <p
                          class="text-[11px] font-semibold tracking-wide text-slate-500"
                        >
                          节点耗时
                        </p>
                        <p class="mt-2 text-sm font-medium text-slate-900">
                          {{
                            selectedTimelineItem.durationMs !== undefined
                              ? `${selectedTimelineItem.durationMs} ms`
                              : "--"
                          }}
                        </p>
                      </div>
                    </div>

                    <div
                      v-if="
                        selectedTimelineItem.errorCode ||
                        selectedTimelineItem.errorDetail ||
                        selectedTimelineItem.recoveryHint
                      "
                      class="rounded-[18px] border border-rose-200 bg-rose-50/90 px-4 py-4"
                    >
                      <p
                        v-if="selectedTimelineItem.errorCode"
                        class="text-sm font-semibold text-rose-700"
                      >
                        {{ selectedTimelineItem.errorCode }}
                      </p>
                      <p
                        v-if="selectedTimelineItem.errorDetail"
                        class="mt-2 whitespace-pre-wrap break-words text-sm leading-6 text-rose-700"
                      >
                        {{ selectedTimelineItem.errorDetail }}
                      </p>
                      <p
                        v-if="selectedTimelineItem.recoveryHint"
                        class="mt-3 rounded-xl bg-white px-3 py-2 text-sm text-rose-800 ring-1 ring-rose-100"
                      >
                        建议：{{ selectedTimelineItem.recoveryHint }}
                      </p>
                    </div>

                    <div class="grid gap-4 xl:grid-cols-2">
                      <div>
                        <p
                          class="text-xs font-semibold tracking-wide text-slate-500"
                        >
                          Input
                        </p>
                        <pre
                          class="mt-3 max-h-80 overflow-auto rounded-[18px] bg-slate-950 px-4 py-4 font-mono text-[11px] leading-5 text-slate-100"
                          >{{
                            JSON.stringify(
                              selectedTimelineItem.input ?? {},
                              null,
                              2,
                            )
                          }}</pre
                        >
                      </div>

                      <div>
                        <p
                          class="text-xs font-semibold tracking-wide text-slate-500"
                        >
                          Output
                        </p>
                        <pre
                          class="mt-3 max-h-80 overflow-auto rounded-[18px] bg-slate-950 px-4 py-4 font-mono text-[11px] leading-5 text-slate-100"
                          >{{
                            JSON.stringify(
                              selectedTimelineItem.output ?? {},
                              null,
                              2,
                            )
                          }}</pre
                        >
                      </div>
                    </div>

                    <div v-if="selectedTimelineItem.logs?.length">
                      <p
                        class="text-xs font-semibold tracking-wide text-slate-500"
                      >
                        原始日志
                      </p>
                      <div class="mt-3 space-y-2">
                        <p
                          v-for="(log, logIndex) in selectedTimelineItem.logs"
                          :key="`${selectedTimelineItem.nodeId}-log-${logIndex}`"
                          class="rounded-xl bg-slate-50 px-3 py-2 font-mono text-[11px] leading-5 text-slate-700 ring-1 ring-slate-200"
                        >
                          [{{ log.level }}] {{ log.message }}
                        </p>
                      </div>
                    </div>
                  </div>

                  <div
                    v-else
                    class="flex min-h-80 items-center justify-center rounded-[18px] border border-dashed border-slate-200 bg-slate-50/80 px-6 text-center text-sm leading-6 text-slate-500"
                  >
                    点击左侧一条节点日志，这里会显示详细输入、输出和错误信息。
                  </div>
                </div>
              </div>
            </template>
          </section>

          <section
            v-if="selectedSummary"
            class="rounded-2xl border border-white/70 bg-white/92 p-5"
          >
            <div class="space-y-4">
              <div>
                <p class="text-xs font-semibold tracking-wide text-slate-500">
                  State Snapshot
                </p>
                <pre
                  class="mt-3 max-h-72 overflow-auto rounded-[18px] bg-slate-950 px-4 py-4 font-mono text-[11px] leading-5 text-slate-100"
                  >{{
                    JSON.stringify(selectedSummary.state ?? {}, null, 2)
                  }}</pre
                >
              </div>

              <div>
                <p class="text-xs font-semibold tracking-wide text-slate-500">
                  Last Output
                </p>
                <pre
                  class="mt-3 max-h-72 overflow-auto rounded-[18px] bg-slate-950 px-4 py-4 font-mono text-[11px] leading-5 text-slate-100"
                  >{{ JSON.stringify(lastOutputValue, null, 2) }}</pre
                >
              </div>
            </div>
          </section>
        </main>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useRouter } from "vue-router";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  fetchWorkflowRunSummary,
  searchWorkflowRuns,
  type WorkflowRunTimelineItem,
  type WorkflowRunSearchItem,
  type WorkflowRunSummary,
} from "@/features/workflow/runner";

const router = useRouter();

const filters = reactive({
  orderNo: "",
  requestId: "",
  runId: "",
  waveNo: "",
});
const results = ref<WorkflowRunSearchItem[]>([]);
const total = ref(0);
const selectedRunId = ref("");
const selectedSummary = ref<WorkflowRunSummary | null>(null);
const selectedTimelineIndex = ref(-1);
const isSearching = ref(false);
const isLoadingSummary = ref(false);
const searchError = ref("");
const summaryError = ref("");

const lastOutputValue = computed(
  () =>
    selectedSummary.value?.timeline[selectedSummary.value.timeline.length - 1]
      ?.output ?? {},
);
const lastOutputSummary = computed(() => {
  const output = lastOutputValue.value;

  if (!output || typeof output !== "object") {
    return String(output ?? "--");
  }

  const pairs = Object.entries(output as Record<string, unknown>)
    .filter(([, value]) =>
      ["string", "number", "boolean"].includes(typeof value),
    )
    .slice(0, 2)
    .map(([key, value]) => `${key}=${String(value)}`);

  return pairs[0] ?? "查看下方 Last Output";
});
const selectedTimelineItem = computed<WorkflowRunTimelineItem | null>(() => {
  if (!selectedSummary.value) {
    return null;
  }

  const item = selectedSummary.value.timeline[selectedTimelineIndex.value];
  return item ?? null;
});

const statusClass = (status: string) => {
  switch (status) {
    case "running":
      return "bg-cyan-50 text-cyan-700";
    case "waiting":
      return "bg-amber-50 text-amber-700";
    case "failed":
      return "bg-rose-50 text-rose-700";
    case "completed":
      return "bg-emerald-50 text-emerald-700";
    default:
      return "bg-slate-100 text-slate-600";
  }
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

const loadRunSummary = async (runId: string) => {
  isLoadingSummary.value = true;
  summaryError.value = "";

  try {
    const summary = await fetchWorkflowRunSummary(runId);
    selectedRunId.value = runId;
    selectedSummary.value = summary;
    selectedTimelineIndex.value = selectDefaultTimelineIndex(summary);
  } catch (error) {
    summaryError.value =
      error instanceof Error ? error.message : "加载运行详情失败";
  } finally {
    isLoadingSummary.value = false;
  }
};

const handleSearch = async () => {
  isSearching.value = true;
  searchError.value = "";
  summaryError.value = "";

  try {
    const response = await searchWorkflowRuns({
      orderNo: filters.orderNo,
      page: 1,
      pageSize: 20,
      requestId: filters.requestId,
      runId: filters.runId,
      waveNo: filters.waveNo,
    });

    results.value = response.items;
    total.value = response.total;

    if (response.items[0]) {
      await loadRunSummary(response.items[0].runId);
    } else {
      selectedRunId.value = "";
      selectedSummary.value = null;
      selectedTimelineIndex.value = -1;
    }
  } catch (error) {
    searchError.value = error instanceof Error ? error.message : "搜索运行失败";
  } finally {
    isSearching.value = false;
  }
};

const handleSelectRun = async (runId: string) => {
  await loadRunSummary(runId);
};

const refreshSelectedRun = async () => {
  if (!selectedRunId.value) {
    return;
  }

  await loadRunSummary(selectedRunId.value);
};

const selectDefaultTimelineIndex = (summary: WorkflowRunSummary) => {
  const failedIndex = summary.timeline.findIndex(
    (item) => item.status === "failed",
  );

  if (failedIndex >= 0) {
    return failedIndex;
  }

  return Math.max(summary.timeline.length - 1, -1);
};

const handleSelectTimelineItem = (index: number) => {
  selectedTimelineIndex.value = index;
};

const handleBack = () => {
  void router.push({ name: "workflow-list" });
};
</script>
