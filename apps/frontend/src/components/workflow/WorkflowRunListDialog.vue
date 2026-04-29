<template>
  <ElDialog
    :model-value="open"
    :title="workflowName || '运行列表'"
    append-to-body
    align-center
    class="workflow-run-list-dialog"
    width="min(92vw, 42rem)"
    @update:model-value="handleOpenChange"
  >
    <!-- <p class="text-sm leading-6 text-slate-500">
      查看当前工作流仍在执行中的任务，并跳转到对应运行详情。
    </p> -->
    <div class="mt-4 max-h-[60vh] overflow-y-auto">
      <div
        v-if="isLoading"
        class="flex items-center justify-center rounded-lg border border-slate-200/80 bg-white px-6 py-12 text-sm text-slate-500"
      >
        加载运行列表中...
      </div>
      <div
        v-else-if="workflowRuns.length === 0"
        class="rounded-lg border border-dashed border-slate-200 bg-white px-6 py-12 text-center"
      >
        <p class="text-sm font-medium text-slate-700">当前没有运行中的任务</p>
        <p class="mt-2 text-sm leading-6 text-slate-500">
          如果任务刚刚结束，刷新后这里的计数和列表也会同步更新。
        </p>
      </div>
      <div v-else class="space-y-3">
        <button
          v-for="run in workflowRuns"
          :key="run.runId"
          type="button"
          class="w-full rounded-lg border border-slate-200/80 bg-white px-4 py-4 text-left shadow-[0_14px_30px_rgba(15,23,42,0.05)] transition-all hover:border-cyan-200 hover:bg-cyan-50/40"
          @click="handleRunSelect(run.runId)"
        >
          <div class="flex items-start justify-between gap-4">
            <div class="min-w-0">
              <p class="text-sm font-semibold text-slate-900">
                {{ run.runId }}
              </p>
              <p class="mt-1 text-sm text-slate-500">
                {{
                  run.currentNodeId
                    ? `当前节点：${run.currentNodeId}`
                    : "等待获取当前节点"
                }}
              </p>
            </div>
            <span class="flex shrink-0 items-center gap-2">
              <ElButton
                native-type="button"
                size="small"
                plain
                @click.stop="handleCopyRunId(run.runId)"
              >
                复制 ID
              </ElButton>
              <span
                class="rounded-full px-2.5 py-1 text-[11px] font-semibold"
                :class="
                  run.status === 'waiting'
                    ? 'bg-amber-50 text-amber-700'
                    : 'bg-cyan-50 text-cyan-700'
                "
              >
                {{ formatRunStatusLabel(run.status) }}
              </span>
            </span>
          </div>
          <div class="mt-4 flex items-center justify-between text-xs text-slate-400">
            <span>启动于 {{ formatRunTime(run.createdAt) }}</span>
            <span>更新于 {{ formatRunTime(run.updatedAt) }}</span>
          </div>
        </button>
      </div>
    </div>
  </ElDialog>
</template>
<script setup lang="ts">
import dayjs from "dayjs";
import { onBeforeUnmount, ref, watch } from "vue";
import { toast } from "@/lib/element-toast";
import {
  subscribeWorkflowEvents,
  type WorkflowStreamNotification,
} from "@/features/workflow/live";
import {
  fetchWorkflowRuns,
  type WorkflowRunListItem,
} from "@/features/workflow/api";
import type { EventSourceSubscription } from "@/lib/sse";
const props = defineProps<{
  open: boolean;
  workflowId: string;
  workflowName?: string;
}>();
const emit = defineEmits<{
  "update:open": [open: boolean];
  "select-run": [runId: string];
}>();
const isLoading = ref(false);
const workflowRuns = ref<WorkflowRunListItem[]>([]);
let workflowEventSubscription: EventSourceSubscription | null = null;
let loadQueued = false;
let latestLoadRequestId = 0;
const closeWorkflowEventSubscription = () => {
  workflowEventSubscription?.close();
  workflowEventSubscription = null;
};
const startWorkflowEventSubscription = (workflowId: string) => {
  closeWorkflowEventSubscription();
  workflowEventSubscription = subscribeWorkflowEvents(workflowId, {
    onEvent: (notification: WorkflowStreamNotification) => {
      if (
        notification.eventType === "stream.connected" ||
        notification.workflowId !== workflowId
      ) {
        return;
      }
      void loadWorkflowRuns({ silent: true });
    },
    onError: () => {
      void loadWorkflowRuns({ silent: true });
    },
  });
};
const handleOpenChange = (nextOpen: boolean) => {
  emit("update:open", nextOpen);
  if (!nextOpen) {
    closeWorkflowEventSubscription();
    workflowRuns.value = [];
  }
};
const formatRunStatusLabel = (status: WorkflowRunListItem["status"]) => {
  if (status === "waiting") {
    return "Waiting";
  }
  return "Running";
};
const formatRunTime = (value: string) => dayjs(value).format("MMM D · HH:mm:ss");
const loadWorkflowRuns = async (
  options: {
    silent?: boolean;
  } = {},
) => {
  if (!props.workflowId) {
    workflowRuns.value = [];
    return;
  }
  if (isLoading.value) {
    loadQueued = true;
    return;
  }
  isLoading.value = true;
  const requestId = ++latestLoadRequestId;
  if (!options.silent) {
    workflowRuns.value = [];
  }
  try {
    const runs = await fetchWorkflowRuns(props.workflowId);
    if (requestId !== latestLoadRequestId) {
      return;
    }
    workflowRuns.value = runs;
  } catch (error) {
    if (!options.silent) {
      toast.error(error instanceof Error ? error.message : "加载运行列表失败");
      emit("update:open", false);
    }
  } finally {
    isLoading.value = false;
    if (loadQueued) {
      loadQueued = false;
      void loadWorkflowRuns({ silent: true });
    }
  }
};
const handleRunSelect = (runId: string) => {
  emit("select-run", runId);
  emit("update:open", false);
};
const handleCopyRunId = async (runId: string) => {
  try {
    await navigator.clipboard.writeText(runId);
    toast.success(`已复制运行 ID：${runId}`);
  } catch {
    toast.error("复制运行 ID 失败");
  }
};
watch(
  () => [props.open, props.workflowId] as const,
  ([open, workflowId], previousValue) => {
    const [previousOpen, previousWorkflowId] = previousValue ?? [false, ""] as const;
    if (!open || !workflowId) {
      closeWorkflowEventSubscription();
      return;
    }
    if (!previousOpen || workflowId !== previousWorkflowId) {
      void loadWorkflowRuns();
      startWorkflowEventSubscription(workflowId);
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  closeWorkflowEventSubscription();
});
</script>
