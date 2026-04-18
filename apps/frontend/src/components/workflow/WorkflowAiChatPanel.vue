<template>
  <aside
    class="pointer-events-auto absolute right-6 top-24 bottom-6 z-10 flex max-h-[calc(100vh-7.5rem)] w-[360px] flex-col overflow-hidden rounded-[20px] bg-white/95 backdrop-blur shadow-sm ring-1 ring-slate-100/50"
    :class="visibilityClass"
  >
    <div class="border-b border-slate-100 px-4 py-4">
      <div class="flex items-start justify-between gap-3">
        <div>
          <p class="text-[14px] font-semibold text-slate-900">
            AI 工作流编辑助手
          </p>
          <p class="mt-1 text-[11px] leading-5 text-slate-500">
            在页面内直接发起协作，Claude 会通过当前 edit session 更新草稿预览。
          </p>
        </div>
        <div class="flex flex-col items-end gap-1">
          <span
            class="rounded-full px-2.5 py-1 text-[11px] font-semibold text-nowrap"
            :class="runnerConnectionClass"
          >
            {{ runnerConnectionLabel }}
          </span>
          <span
            class="rounded-full px-2.5 py-1 text-[11px] font-semibold text-nowrap"
            :class="threadStatusClass"
          >
            {{ threadStatusLabel }}
          </span>
        </div>
      </div>
    </div>

    <div class="border-b border-slate-100 px-4 py-3">
      <div class="grid grid-cols-1 gap-2 text-[11px] text-slate-500">
        <!-- <div class="rounded-[14px] border border-slate-200/80 bg-white px-3 py-2">
          <div class="font-semibold tracking-wide">
            session_id: {{ sessionId || "(创建中)" }}
          </div>
          <div class="mt-1 font-semibold tracking-wide break-all">
            runner_base_url: {{ runnerBaseUrl }}
          </div>
        </div> -->
        <div class="rounded-[14px] border border-slate-200/80 bg-slate-50/80 px-3 py-2">
          <p class="font-semibold tracking-wide text-slate-500">最近同步</p>
          <p class="mt-1 text-sm font-medium text-slate-900">
            {{ previewSyncLabel }}
          </p>
          <p class="mt-1">
            Claude 事件流：{{ gatewayConnectionLabel }}
          </p>
          <p v-if="claudeSessionId" class="mt-1 break-all">
            claude_session_id: {{ claudeSessionId }}
          </p>
        </div>
        <div
          v-if="combinedError"
          class="rounded-[14px] border border-rose-100 bg-rose-50 px-3 py-2 text-rose-700"
        >
          {{ combinedError }}
        </div>
      </div>
    </div>

    <div
      ref="messageContainerRef"
      class="min-h-0 flex-1 space-y-3 overflow-y-auto bg-slate-50/50 px-4 py-4"
    >
      <template v-if="threadSnapshot.messages.length > 0">
        <article
          v-for="message in threadSnapshot.messages"
          :key="message.id"
          class="rounded-[16px] px-3 py-3 text-[13px] leading-6 shadow-sm"
          :class="messageClassMap[message.role]"
        >
          <div class="flex items-center justify-between gap-3">
            <p class="text-[11px] font-semibold uppercase tracking-[0.22em]">
              {{ messageRoleLabelMap[message.role] }}
            </p>
            <p class="text-[11px] opacity-70">
              {{ formatMessageTime(message.createdAt) }}
            </p>
          </div>
          <p class="mt-2 whitespace-pre-wrap break-words">
            {{ message.content || "..." }}
          </p>
        </article>
      </template>

      <div
        v-else
        class="rounded-[16px] border border-dashed border-slate-200 bg-white/85 px-4 py-5 text-[13px] leading-6 text-slate-500"
      >
        先描述你的改动需求，例如“新增一个审核分支，并把失败结果汇总到 respond 节点”。
      </div>
    </div>

    <div class="border-t border-slate-100 bg-white px-4 py-4">
      <textarea
        v-model="draftMessage"
        class="min-h-[92px] w-full resize-none rounded-[16px] border border-slate-200 bg-slate-50 px-3 py-3 text-[13px] leading-6 text-slate-800 outline-none transition-colors placeholder:text-slate-400 focus:border-slate-300 focus:bg-white"
        :disabled="isComposerDisabled"
        placeholder="描述你希望 Claude 帮你调整的工作流内容"
        @keydown.enter.exact.prevent="handleSend"
      />
      <div class="mt-3 flex items-center justify-between gap-3">
        <p class="text-[11px] leading-5 text-slate-500">
          Enter 发送，Shift + Enter 换行
        </p>
        <div class="flex items-center gap-2">
          <Button
            variant="ghost"
            class="h-9 rounded-full px-3 text-sm font-medium text-slate-600 hover:bg-slate-100"
            :disabled="!isRunning"
            @click="handleCancel"
          >
            停止
          </Button>
          <Button
            class="h-9 rounded-full bg-slate-900 px-4 text-sm font-medium text-white hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="isComposerDisabled || !draftMessage.trim()"
            @click="handleSend"
          >
            {{ isRunning ? "协作中..." : "发送" }}
          </Button>
        </div>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import {
  cancelAiThreadTurn,
  fetchAiThreadSnapshot,
  persistAiThreadSnapshot,
  readPersistedAiThreadSnapshot,
  resolveAiChatStorage,
  sendAiThreadMessage,
  subscribeAiThreadEvents,
  type AiChatMessageRole,
  type AiThreadSnapshot,
} from "@/features/workflow/ai-chat";
import type { EventSourceSubscription } from "@/lib/sse";

const props = defineProps<{
  runnerBaseUrl: string;
  runnerConnectionLabel: string;
  runnerConnectionState: "idle" | "connecting" | "live" | "reconnecting";
  runnerPreviewUpdatedAt?: string;
  sessionError?: string;
  sessionId: string;
  visibilityClass?: string;
  workflowId?: string;
}>();

const storage = resolveAiChatStorage();
const messageContainerRef = ref<HTMLElement | null>(null);
const draftMessage = ref("");
const gatewayError = ref("");
const threadSnapshot = ref<AiThreadSnapshot>({
  editSessionId: "",
  status: "idle",
  messages: [],
});
const gatewayConnectionState = ref<"idle" | "connecting" | "live" | "reconnecting">("idle");
let threadEventSubscription: EventSourceSubscription | null = null;

const messageRoleLabelMap: Record<AiChatMessageRole, string> = {
  user: "需求",
  assistant: "Claude",
  "tool-status": "工具",
  error: "错误",
};

const messageClassMap: Record<AiChatMessageRole, string> = {
  user: "bg-slate-900 text-white",
  assistant: "bg-white text-slate-800 ring-1 ring-slate-100",
  "tool-status": "bg-amber-50 text-amber-900 ring-1 ring-amber-100",
  error: "bg-rose-50 text-rose-800 ring-1 ring-rose-100",
};

const isRunning = computed(() => threadSnapshot.value.status === "running");
const claudeSessionId = computed(() => threadSnapshot.value.claudeSessionId ?? "");
const gatewayConnectionLabel = computed(() => {
  switch (gatewayConnectionState.value) {
    case "connecting":
      return "连接中";
    case "live":
      return "已连接";
    case "reconnecting":
      return "重连中";
    default:
      return "未启动";
  }
});
const threadStatusLabel = computed(() => {
  switch (threadSnapshot.value.status) {
    case "running":
      return "Claude 协作中";
    case "error":
      return "Claude 需重试";
    default:
      return "Claude 已就绪";
  }
});
const threadStatusClass = computed(() => {
  switch (threadSnapshot.value.status) {
    case "running":
      return "bg-cyan-50 text-cyan-700";
    case "error":
      return "bg-rose-50 text-rose-700";
    default:
      return "bg-emerald-50 text-emerald-700";
  }
});
const runnerConnectionClass = computed(() => {
  switch (props.runnerConnectionState) {
    case "live":
      return "bg-sky-50 text-sky-700";
    case "reconnecting":
      return "bg-amber-50 text-amber-700";
    default:
      return "bg-slate-100 text-slate-600";
  }
});
const previewSyncLabel = computed(() => {
  const timestamp =
    threadSnapshot.value.lastPreviewSyncAt ?? props.runnerPreviewUpdatedAt ?? "";
  if (!timestamp) {
    return "尚未同步";
  }

  return new Date(timestamp).toLocaleString("zh-CN");
});
const combinedError = computed(() => gatewayError.value || props.sessionError || "");
const isComposerDisabled = computed(() => !props.sessionId || isRunning.value);

const formatMessageTime = (value: string) =>
  new Date(value).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

const persistSnapshot = (snapshot: AiThreadSnapshot) => {
  if (!snapshot.editSessionId) {
    return;
  }

  persistAiThreadSnapshot(storage, snapshot);
};

const applySnapshot = (snapshot: AiThreadSnapshot) => {
  threadSnapshot.value = snapshot;
  persistSnapshot(snapshot);
  void nextTick(() => {
    messageContainerRef.value?.scrollTo({
      top: messageContainerRef.value.scrollHeight,
      behavior: "smooth",
    });
  });
};

const closeEventStream = () => {
  threadEventSubscription?.close();
  threadEventSubscription = null;
};

const ensureEventStream = (sessionId: string) => {
  closeEventStream();
  gatewayConnectionState.value = "connecting";
  threadEventSubscription = subscribeAiThreadEvents(sessionId, {
    onOpen: () => {
      gatewayConnectionState.value = "live";
    },
    onError: () => {
      gatewayConnectionState.value = "reconnecting";
    },
    onEvent: (event) => {
      gatewayError.value = "";
      applySnapshot(event.snapshot);
    },
  });
};

const hydrateThread = async (sessionId: string) => {
  if (!sessionId) {
    threadSnapshot.value = {
      editSessionId: "",
      status: "idle",
      messages: [],
    };
    closeEventStream();
    gatewayConnectionState.value = "idle";
    return;
  }

  const cachedSnapshot = readPersistedAiThreadSnapshot(storage, sessionId);
  if (cachedSnapshot) {
    applySnapshot(cachedSnapshot);
  } else {
    threadSnapshot.value = {
      editSessionId: sessionId,
      status: "idle",
      messages: [],
    };
  }

  try {
    const snapshot = await fetchAiThreadSnapshot(sessionId);
    applySnapshot(snapshot);
    ensureEventStream(sessionId);
  } catch (error) {
    gatewayError.value =
      error instanceof Error ? error.message : "获取 AI 会话失败";
    toast.error(gatewayError.value);
  }
};

const handleSend = async () => {
  const message = draftMessage.value.trim();
  if (!message || !props.sessionId || isRunning.value) {
    return;
  }

  try {
    gatewayError.value = "";
    const snapshot = await sendAiThreadMessage(props.sessionId, {
      message,
      runnerBaseUrl: props.runnerBaseUrl,
      workflowId: props.workflowId,
    });
    applySnapshot(snapshot);
    draftMessage.value = "";
    ensureEventStream(props.sessionId);
  } catch (error) {
    gatewayError.value =
      error instanceof Error ? error.message : "发送 AI 消息失败";
    toast.error(gatewayError.value);
  }
};

const handleCancel = async () => {
  if (!props.sessionId || !isRunning.value) {
    return;
  }

  try {
    const snapshot = await cancelAiThreadTurn(props.sessionId);
    applySnapshot(snapshot);
  } catch (error) {
    gatewayError.value =
      error instanceof Error ? error.message : "停止 AI 协作失败";
    toast.error(gatewayError.value);
  }
};

watch(
  () => props.sessionId,
  (sessionId) => {
    void hydrateThread(sessionId);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  closeEventStream();
});
</script>
