<template>
  <section
    class="min-h-screen bg-[linear-gradient(180deg,#f8fafc_0%,#eef6ff_48%,#f8fafc_100%)] text-slate-900"
  >
    <div class="mx-auto max-w-7xl px-5 py-6 sm:px-8 lg:px-10">
      <header class="flex flex-wrap items-center justify-between gap-4">
        <div>
          <p
            class="text-[11px] font-semibold uppercase tracking-[0.24em] text-cyan-700"
          >
            Troubleshoot
          </p>
          <h1 class="mt-2 text-3xl font-semibold tracking-tight text-slate-950">
            运行排障工作台
          </h1>
          <p class="mt-2 max-w-2xl text-sm leading-6 text-slate-500">
            按 `runId`、`requestId`、订单号或波次号定位运行记录，直接查看
            timeline、错误提示和人工补录备注。
          </p>
        </div>

        <Button
          variant="outline"
          class="rounded-full border-slate-200 bg-white px-4 text-slate-700 hover:border-cyan-200 hover:bg-cyan-50 hover:text-cyan-700"
          @click="handleBack"
        >
          返回工作流列表
        </Button>
      </header>

      <section
        class="mt-6 rounded-[28px] border border-white/70 bg-white/90 p-5 shadow-[0_24px_70px_rgba(15,23,42,0.08)] backdrop-blur"
      >
        <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
          <Input
            v-model="filters.runId"
            class="h-11 rounded-2xl border-slate-200 bg-slate-50 px-4"
            placeholder="Run ID"
          />
          <Input
            v-model="filters.requestId"
            class="h-11 rounded-2xl border-slate-200 bg-slate-50 px-4"
            placeholder="Request ID"
          />
          <Input
            v-model="filters.orderNo"
            class="h-11 rounded-2xl border-slate-200 bg-slate-50 px-4"
            placeholder="订单号"
          />
          <Input
            v-model="filters.waveNo"
            class="h-11 rounded-2xl border-slate-200 bg-slate-50 px-4"
            placeholder="波次号"
          />
          <Button
            class="h-11 rounded-2xl bg-slate-950 text-white hover:bg-slate-800"
            :disabled="isSearching"
            @click="handleSearch"
          >
            {{ isSearching ? "搜索中..." : "搜索运行" }}
          </Button>
        </div>
        <p v-if="searchError" class="mt-3 text-sm text-rose-600">
          {{ searchError }}
        </p>
      </section>

      <div class="mt-6 grid gap-6 xl:grid-cols-[360px_minmax(0,1fr)]">
        <aside
          class="rounded-[28px] border border-white/70 bg-white/92 p-4 shadow-[0_24px_70px_rgba(15,23,42,0.08)]"
        >
          <div class="flex items-center justify-between gap-3 px-2 pb-3">
            <div>
              <p class="text-xs font-semibold tracking-wide text-slate-500">
                搜索结果
              </p>
              <p class="mt-1 text-sm text-slate-400">
                {{ total }} 条记录
              </p>
            </div>
          </div>

          <div
            v-if="results.length === 0"
            class="flex min-h-60 items-center justify-center rounded-[22px] border border-dashed border-slate-200 bg-slate-50/80 px-6 text-center text-sm leading-6 text-slate-500"
          >
            输入检索条件后即可定位运行记录。
          </div>

          <div v-else class="space-y-3">
            <button
              v-for="item in results"
              :key="item.runId"
              type="button"
              class="w-full rounded-[22px] border px-4 py-4 text-left transition"
              :class="
                item.runId === selectedRunId
                  ? 'border-cyan-200 bg-cyan-50/80 shadow-[0_16px_40px_rgba(6,182,212,0.12)]'
                  : 'border-slate-200 bg-slate-50/70 hover:border-cyan-100 hover:bg-white'
              "
              @click="handleSelectRun(item.runId)"
            >
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <p class="truncate text-sm font-semibold text-slate-900">
                    {{ item.runId }}
                  </p>
                  <p class="mt-1 text-xs text-slate-500">
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

              <div class="mt-3 space-y-1 text-xs text-slate-500">
                <p v-if="item.requestId">Request ID: {{ item.requestId }}</p>
                <p v-if="item.orderNo">订单号: {{ item.orderNo }}</p>
                <p v-if="item.waveNo">波次号: {{ item.waveNo }}</p>
                <p>开始时间: {{ formatDateTime(item.startedAt) }}</p>
                <p v-if="item.durationMs !== undefined">
                  总耗时: {{ item.durationMs }} ms
                </p>
              </div>
            </button>
          </div>
        </aside>

        <main class="space-y-6">
          <section
            class="rounded-[28px] border border-white/70 bg-white/92 p-5 shadow-[0_24px_70px_rgba(15,23,42,0.08)]"
          >
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p class="text-xs font-semibold tracking-wide text-slate-500">
                  运行详情
                </p>
                <p class="mt-1 text-sm text-slate-400">
                  {{
                    selectedSummary
                      ? `${selectedSummary.runId} · ${selectedSummary.workflowKey}`
                      : "选择左侧运行记录查看详情"
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
              搜索后点击一条运行记录，这里会展示完整 timeline、回放和错误提示。
            </div>

            <template v-else>
              <div class="mt-4 grid gap-3 md:grid-cols-4">
                <div class="rounded-2xl bg-slate-50 px-4 py-3">
                  <p class="text-[11px] font-semibold tracking-wide text-slate-500">
                    状态
                  </p>
                  <p class="mt-2 text-base font-semibold text-slate-900">
                    {{ selectedSummary.status }}
                  </p>
                </div>
                <div class="rounded-2xl bg-slate-50 px-4 py-3">
                  <p class="text-[11px] font-semibold tracking-wide text-slate-500">
                    当前节点
                  </p>
                  <p class="mt-2 text-base font-semibold text-slate-900">
                    {{ selectedSummary.currentNodeId ?? "--" }}
                  </p>
                </div>
                <div class="rounded-2xl bg-slate-50 px-4 py-3">
                  <p class="text-[11px] font-semibold tracking-wide text-slate-500">
                    Timeline Steps
                  </p>
                  <p class="mt-2 text-base font-semibold text-slate-900">
                    {{ selectedSummary.timeline.length }}
                  </p>
                </div>
                <div class="rounded-2xl bg-slate-50 px-4 py-3">
                  <p class="text-[11px] font-semibold tracking-wide text-slate-500">
                    最近输出
                  </p>
                  <p class="mt-2 truncate text-base font-semibold text-slate-900">
                    {{ lastOutputSummary }}
                  </p>
                </div>
              </div>

              <div class="mt-5">
                <WorkflowRunTimelineDetail
                  :summary="selectedSummary"
                  :workflow-key="selectedSummary.workflowKey"
                />
              </div>
            </template>
          </section>

          <section
            v-if="selectedSummary"
            class="rounded-[28px] border border-white/70 bg-white/92 p-5 shadow-[0_24px_70px_rgba(15,23,42,0.08)]"
          >
            <div class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_340px]">
              <div class="space-y-4">
                <div>
                  <p class="text-xs font-semibold tracking-wide text-slate-500">
                    State Snapshot
                  </p>
                  <pre
                    class="mt-3 max-h-72 overflow-auto rounded-[18px] bg-slate-950 px-4 py-4 font-mono text-[11px] leading-5 text-slate-100"
                    >{{ JSON.stringify(selectedSummary.state ?? {}, null, 2) }}</pre
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

              <div class="rounded-[22px] border border-slate-200 bg-slate-50/80 p-4">
                <p class="text-sm font-semibold text-slate-900">人工补录</p>
                <p class="mt-2 text-xs leading-6 text-slate-500">
                  给 timeline 节点补一条处理记录，方便现场交接和后续复盘。
                </p>

                <label class="mt-4 block text-xs font-semibold text-slate-500">
                  节点
                </label>
                <select
                  v-model="manualPatch.nodeId"
                  class="mt-2 h-11 w-full rounded-2xl border border-slate-200 bg-white px-3 text-sm text-slate-900 outline-none transition focus:border-cyan-300"
                >
                  <option
                    v-for="item in selectedSummary.timeline"
                    :key="`manual-${item.nodeId}`"
                    :value="item.nodeId"
                  >
                    {{ item.nodeId }} · {{ item.nodeType }}
                  </option>
                </select>

                <label class="mt-4 block text-xs font-semibold text-slate-500">
                  处理人
                </label>
                <Input
                  v-model="manualPatch.operator"
                  class="mt-2 h-11 rounded-2xl border-slate-200 bg-white px-3"
                  placeholder="张工"
                />

                <label class="mt-4 block text-xs font-semibold text-slate-500">
                  备注
                </label>
                <textarea
                  v-model="manualPatch.note"
                  class="mt-2 min-h-30 w-full rounded-2xl border border-slate-200 bg-white px-3 py-3 text-sm text-slate-900 outline-none transition focus:border-cyan-300"
                  placeholder="例如：人工确认外部系统已补单，等待下一次回调验证。"
                />

                <p v-if="manualPatchError" class="mt-3 text-sm text-rose-600">
                  {{ manualPatchError }}
                </p>

                <Button
                  class="mt-4 w-full rounded-2xl bg-slate-950 text-white hover:bg-slate-800"
                  :disabled="isSubmittingManualPatch"
                  @click="handleSubmitManualPatch"
                >
                  {{
                    isSubmittingManualPatch ? "提交中..." : "写入人工补录"
                  }}
                </Button>
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
import { toast } from "vue-sonner";

import WorkflowRunTimelineDetail from "@/components/workflow/WorkflowRunTimelineDetail.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  fetchWorkflowRunSummary,
  manualPatchWorkflowRun,
  searchWorkflowRuns,
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
const isSearching = ref(false);
const isLoadingSummary = ref(false);
const isSubmittingManualPatch = ref(false);
const searchError = ref("");
const summaryError = ref("");
const manualPatchError = ref("");
const manualPatch = reactive({
  nodeId: "",
  note: "",
  operator: "现场工程师",
});

const lastOutputValue = computed(
  () => selectedSummary.value?.timeline[selectedSummary.value.timeline.length - 1]?.output ?? {},
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

const selectDefaultManualPatchNode = (summary: WorkflowRunSummary) => {
  const failedNode = summary.timeline.find((item) => item.status === "failed");
  manualPatch.nodeId =
    failedNode?.nodeId ??
    summary.currentNodeId ??
    summary.timeline[summary.timeline.length - 1]?.nodeId ??
    "";
};

const loadRunSummary = async (runId: string) => {
  isLoadingSummary.value = true;
  summaryError.value = "";

  try {
    const summary = await fetchWorkflowRunSummary(runId);
    selectedRunId.value = runId;
    selectedSummary.value = summary;
    selectDefaultManualPatchNode(summary);
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

const handleSubmitManualPatch = async () => {
  if (!selectedRunId.value || !selectedSummary.value) {
    return;
  }

  isSubmittingManualPatch.value = true;
  manualPatchError.value = "";

  try {
    const summary = await manualPatchWorkflowRun(selectedRunId.value, {
      nodeId: manualPatch.nodeId,
      note: manualPatch.note,
      operator: manualPatch.operator,
    });
    selectedSummary.value = summary;
    manualPatch.note = "";
    toast.success("人工补录已写入 timeline");
  } catch (error) {
    manualPatchError.value =
      error instanceof Error ? error.message : "写入人工补录失败";
  } finally {
    isSubmittingManualPatch.value = false;
  }
};

const handleBack = () => {
  void router.push({ name: "workflow-list" });
};
</script>
