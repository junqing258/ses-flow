<template>
  <section
    class="relative min-h-screen overflow-hidden bg-[#fcfcfb] text-slate-900"
  >
    <div class="pointer-events-none absolute inset-0 overflow-hidden">
      <div
        class="absolute left-[-8%] top-[-10%] h-72 w-72 rounded-full bg-[radial-gradient(circle,_rgba(34,211,238,0.16),_rgba(34,211,238,0))]"
      />
      <div
        class="absolute right-[-4%] top-[18%] h-80 w-80 rounded-full bg-[radial-gradient(circle,_rgba(250,204,21,0.14),_rgba(250,204,21,0))]"
      />
      <div
        class="absolute inset-x-0 top-0 h-64 bg-[linear-gradient(180deg,rgba(255,255,255,0.96),rgba(255,255,255,0))]"
      />
    </div>

    <div
      class="relative mx-auto flex min-h-screen w-full max-w-6xl flex-col px-5 pb-20 pt-6 sm:px-8 lg:px-12"
    >
      <header class="flex items-center justify-between gap-4">
        <div class="flex items-center gap-3">
          <div
            class="flex h-10 w-10 items-center justify-center rounded-2xl border border-white/80 bg-white/90 shadow-[0_8px_24px_rgba(15,23,42,0.08)]"
          >
            <Workflow class="h-5 w-5 text-slate-900" />
          </div>
          <div>
            <p
              class="text-[10px] font-semibold uppercase tracking-[0.32em] text-slate-400"
            >
              Builder
            </p>
            <h1 class="text-lg font-semibold tracking-tight text-slate-900">
              Workflow Builder
            </h1>
          </div>
        </div>

        <div class="flex items-center gap-3">
          <Button
            variant="outline"
            class="h-10 rounded-full border-slate-200/80 bg-white/90 px-4 text-sm font-medium text-slate-700 shadow-[0_10px_30px_rgba(15,23,42,0.05)] hover:border-cyan-200 hover:bg-cyan-50 hover:text-cyan-700"
            @click="openHelp"
          >
            <BookOpen class="h-4 w-4" />
            帮助文档
          </Button>
          <div
            class="hidden rounded-full border border-slate-200/80 bg-white/90 px-3 py-1.5 text-xs font-medium text-slate-500 shadow-[0_10px_30px_rgba(15,23,42,0.05)] sm:flex"
          >
            {{ workflowSummaries.length }} workflows ·
            {{ templateWorkflows.length }} templates
          </div>
        </div>
      </header>

      <main class="flex flex-1 flex-col items-center">
        <section
          class="mx-auto mt-16 flex max-w-2xl flex-col items-center text-center sm:mt-24"
        >
          <div
            class="mb-6 inline-flex items-center gap-2 rounded-full border border-cyan-100 bg-cyan-50/80 px-4 py-1.5 text-xs font-semibold uppercase tracking-[0.22em] text-cyan-700"
          >
            Workflow Studio
          </div>
          <h2
            class="max-w-xl text-3xl font-semibold tracking-tight text-slate-950 sm:text-[42px] sm:leading-[1.05]"
          >
            Create a workflow
          </h2>
          <p
            class="mt-4 max-w-lg text-sm leading-7 text-slate-500 sm:text-base"
          >
            Build a chat agent workflow with custom logic, reusable nodes, and
            publish-ready orchestration.
          </p>
          <Button
            class="mt-8 h-11 rounded-full bg-slate-950 px-6 text-sm font-medium text-white shadow-[0_16px_36px_rgba(15,23,42,0.16)] hover:bg-slate-800"
            @click="handleCreate"
          >
            <Plus class="h-4 w-4" />
            Create
          </Button>
        </section>

        <section class="mt-14 w-full max-w-4xl sm:mt-18">
          <Tabs v-model="activeTab" class="w-full">
            <TabsList
              class="h-auto rounded-2xl border border-slate-200/80 bg-slate-100/80 p-1.5 shadow-[inset_0_1px_0_rgba(255,255,255,0.8)]"
            >
              <TabsTrigger
                value="drafts"
                class="rounded-xl px-4 py-2 text-sm font-medium text-slate-500 data-[state=active]:bg-white data-[state=active]:text-slate-900 data-[state=active]:shadow-[0_8px_20px_rgba(15,23,42,0.08)]"
              >
                Drafts
              </TabsTrigger>
              <TabsTrigger
                value="templates"
                class="rounded-xl px-4 py-2 text-sm font-medium text-slate-500 data-[state=active]:bg-white data-[state=active]:text-slate-900 data-[state=active]:shadow-[0_8px_20px_rgba(15,23,42,0.08)]"
              >
                Templates
              </TabsTrigger>
            </TabsList>

            <TabsContent value="drafts" class="mt-5">
              <div
                v-if="isLoadingWorkflows"
                class="rounded-[24px] border border-slate-200/80 bg-white/92 px-6 py-12 text-center text-sm text-slate-500 shadow-[0_20px_45px_rgba(15,23,42,0.06)]"
              >
                Loading workflows...
              </div>
              <div
                v-else-if="draftWorkflows.length === 0"
                class="rounded-[24px] border border-dashed border-slate-200 bg-white/70 px-6 py-12 text-center"
              >
                <p class="text-sm font-medium text-slate-700">
                  还没有已保存的工作流
                </p>
                <p class="mt-2 text-sm leading-6 text-slate-500">
                  先创建并发布一个工作流，列表页就会从数据库里展示出来。
                </p>
              </div>
              <div v-else class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                <article
                  v-for="workflow in draftWorkflows"
                  :key="workflow.id"
                  class="group rounded-[24px] border border-slate-200/80 bg-white/92 p-4 shadow-[0_20px_45px_rgba(15,23,42,0.06)] transition-all duration-200 hover:-translate-y-0.5 hover:border-cyan-200 hover:shadow-[0_24px_50px_rgba(15,23,42,0.1)]"
                >
                  <button
                    type="button"
                    class="w-full text-left"
                    @click="openWorkflow(workflow.id)"
                  >
                    <div class="flex items-start justify-between gap-3">
                      <div
                        class="flex h-10 w-10 items-center justify-center rounded-2xl text-slate-900"
                        :class="workflow.iconClass"
                      >
                        <component :is="workflow.icon" class="h-4 w-4" />
                      </div>
                      <span
                        class="rounded-full px-2.5 py-1 text-[11px] font-semibold"
                        :class="workflow.statusClass"
                      >
                        {{ workflow.status }}
                      </span>
                    </div>

                    <div class="mt-8">
                      <h3
                        class="text-base font-semibold tracking-tight text-slate-900"
                      >
                        {{ workflow.name }}
                      </h3>
                      <p class="mt-2 text-sm leading-6 text-slate-500">
                        {{ workflow.description }}
                      </p>
                    </div>

                    <div
                      class="mt-8 flex items-center justify-between text-xs text-slate-400"
                    >
                      <div class="flex items-center gap-2">
                        <Clock3 class="h-3.5 w-3.5" />
                        <span>{{ workflow.updatedAt }}</span>
                      </div>
                      <span>{{ workflow.owner }}</span>
                    </div>
                  </button>

                  <div
                    class="mt-4 flex items-center justify-between gap-3 rounded-[18px] border border-slate-100 bg-slate-50/80 px-3 py-2.5"
                  >
                    <div>
                      <p
                        class="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-400"
                      >
                        Active Runs
                      </p>
                      <p class="mt-1 text-sm font-semibold text-slate-900">
                        {{ workflow.runningRunCount }}
                        {{ workflow.runningRunCount === 1 ? "task" : "tasks" }}
                      </p>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      class="rounded-full border-slate-200 bg-white px-3.5 text-slate-700 hover:border-cyan-200 hover:bg-cyan-50 hover:text-cyan-700 disabled:border-slate-200 disabled:bg-slate-100 disabled:text-slate-400"
                      :disabled="workflow.runningRunCount === 0"
                      @click="openRunList(workflow)"
                    >
                      <LoaderCircle class="h-3.5 w-3.5" />
                      查看运行
                      <span
                        class="inline-flex min-w-[1.35rem] items-center justify-center rounded-full bg-slate-900 px-1.5 py-0.5 text-[10px] font-semibold leading-none text-white"
                      >
                        {{ workflow.runningRunCount }}
                      </span>
                    </Button>
                  </div>
                </article>
              </div>
            </TabsContent>

            <TabsContent value="templates" class="mt-5">
              <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                <article
                  v-for="template in templateWorkflows"
                  :key="template.id"
                  class="rounded-[24px] border border-slate-200/80 bg-white/92 p-4 shadow-[0_20px_45px_rgba(15,23,42,0.06)]"
                >
                  <div class="flex items-start justify-between gap-3">
                    <div
                      class="flex h-10 w-10 items-center justify-center rounded-2xl text-slate-900"
                      :class="template.iconClass"
                    >
                      <component :is="template.icon" class="h-4 w-4" />
                    </div>
                    <span
                      class="rounded-full bg-slate-100 px-2.5 py-1 text-[11px] font-semibold text-slate-500"
                    >
                      Template
                    </span>
                  </div>

                  <div class="mt-8">
                    <h3
                      class="text-base font-semibold tracking-tight text-slate-900"
                    >
                      {{ template.name }}
                    </h3>
                    <p class="mt-2 text-sm leading-6 text-slate-500">
                      {{ template.description }}
                    </p>
                  </div>

                  <div class="mt-8 flex items-center justify-between">
                    <span class="text-xs text-slate-400">{{
                      template.category
                    }}</span>
                    <Button
                      variant="outline"
                      size="sm"
                      class="rounded-full border-slate-200 bg-white px-3.5 text-slate-700 hover:border-cyan-200 hover:bg-cyan-50 hover:text-cyan-700"
                      @click="useTemplate(template.id)"
                    >
                      Use template
                    </Button>
                  </div>
                </article>
              </div>
            </TabsContent>
          </Tabs>
        </section>
      </main>
    </div>

    <WorkflowRunListDialog
      :open="isRunListOpen"
      :workflow-id="selectedWorkflowForRuns?.id ?? ''"
      :workflow-name="selectedWorkflowForRuns?.name"
      @update:open="handleRunListOpenChange"
      @select-run="openWorkflowRun"
    />
  </section>
</template>

<script setup lang="ts">
import dayjs from "dayjs";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  BookOpen,
  Clock3,
  GitBranchPlus,
  LoaderCircle,
  Plus,
  Sparkles,
  Wand2,
  Workflow,
} from "lucide-vue-next";
import { useRouter } from "vue-router";
import { toast } from "vue-sonner";

import WorkflowRunListDialog from "@/components/workflow/WorkflowRunListDialog.vue";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { subscribeWorkflowsEvents } from "@/features/workflow/live";
import {
  fetchWorkflowList,
  type WorkflowSummary,
} from "@/features/workflow/api";
import type { EventSourceSubscription } from "@/lib/sse";

type WorkflowTabId = "drafts" | "templates";

interface WorkflowListItem {
  id: string;
  name: string;
  description: string;
  updatedAt: string;
  owner: string;
  runningRunCount: number;
  status: string;
  statusClass: string;
  icon: typeof Workflow;
  iconClass: string;
}

interface WorkflowTemplateItem {
  id: string;
  name: string;
  description: string;
  category: string;
  icon: typeof Workflow;
  iconClass: string;
}

const router = useRouter();
const activeTab = ref<WorkflowTabId>("drafts");
const workflowSummaries = ref<WorkflowSummary[]>([]);
const isLoadingWorkflows = ref(false);
const isRunListOpen = ref(false);
const selectedWorkflowForRuns = ref<WorkflowListItem | null>(null);
let workflowsEventSubscription: EventSourceSubscription | null = null;
let workflowListRefreshQueued = false;

const draftWorkflows = computed<WorkflowListItem[]>(() =>
  workflowSummaries.value.map((workflow, index) => ({
    id: workflow.workflowId,
    name: workflow.name,
    description: `${workflow.workflowId} · ${workflow.version}`,
    updatedAt: dayjs(workflow.updatedAt).format("MMM D · HH:mm"),
    owner: workflow.ownerName ?? "Unassigned",
    runningRunCount: workflow.runningRunCount,
    status: workflow.status === "published" ? "Published" : "Draft",
    statusClass:
      workflow.status === "published"
        ? "bg-emerald-50 text-emerald-700"
        : "bg-amber-50 text-amber-700",
    icon: [GitBranchPlus, Sparkles, Workflow][index % 3] ?? Workflow,
    iconClass:
      ["bg-[#ffe082]", "bg-[#c4f1f9]", "bg-[#d9f99d]"][index % 3] ??
      "bg-[#d9f99d]",
  })),
);

const templateWorkflows: WorkflowTemplateItem[] = [
  {
    id: "template-chat-agent",
    name: "Chat agent starter",
    description:
      "Begin with an LLM-first workflow that includes message intake, tool execution, and answer formatting.",
    category: "Conversation",
    icon: Wand2,
    iconClass: "bg-[#dbeafe]",
  },
  {
    id: "template-webhook-ops",
    name: "Webhook automation",
    description:
      "A template for inbound events, condition branches, and outbound callback delivery.",
    category: "Automation",
    icon: GitBranchPlus,
    iconClass: "bg-[#fde68a]",
  },
  {
    id: "template-monitoring",
    name: "Incident triage",
    description:
      "Start from a monitoring workflow with classification, escalation, and stateful follow-up.",
    category: "Operations",
    icon: Sparkles,
    iconClass: "bg-[#c7f9cc]",
  },
];

const closeWorkflowsEventSubscription = () => {
  workflowsEventSubscription?.close();
  workflowsEventSubscription = null;
};

const ensureWorkflowsEventSubscription = () => {
  if (workflowsEventSubscription) {
    return;
  }

  workflowsEventSubscription = subscribeWorkflowsEvents({
    onEvent: (notification) => {
      if (notification.eventType === "stream.connected") {
        return;
      }

      void loadWorkflowList({ silent: true });
    },
    onError: () => {
      void loadWorkflowList({ silent: true });
    },
  });
};

const loadWorkflowList = async (
  options: {
    silent?: boolean;
  } = {},
) => {
  if (isLoadingWorkflows.value) {
    workflowListRefreshQueued = true;
    return;
  }

  isLoadingWorkflows.value = true;

  try {
    workflowSummaries.value = await fetchWorkflowList();
  } catch (error) {
    if (!options.silent) {
      toast.error(error instanceof Error ? error.message : "加载工作流列表失败");
    }
  } finally {
    isLoadingWorkflows.value = false;

    if (workflowListRefreshQueued) {
      workflowListRefreshQueued = false;
      void loadWorkflowList({ silent: true });
    }
  }
};

onMounted(() => {
  void loadWorkflowList();
  ensureWorkflowsEventSubscription();
});

onBeforeUnmount(() => {
  closeWorkflowsEventSubscription();
});

const handleCreate = () => {
  void router.push({ name: "workflow-new" });
};

const openHelp = () => {
  void router.push({ path: "/help" });
};

const openWorkflow = (workflowId: string) => {
  void router.push({
    name: "workflow-editor",
    params: {
      id: workflowId,
    },
  });
};

const handleRunListOpenChange = (open: boolean) => {
  isRunListOpen.value = open;

  if (!open) {
    selectedWorkflowForRuns.value = null;
  }
};

const openRunList = async (workflow: WorkflowListItem) => {
  if (workflow.runningRunCount === 0) {
    return;
  }

  selectedWorkflowForRuns.value = workflow;
  isRunListOpen.value = true;
};

const openWorkflowRun = (runId: string) => {
  if (!selectedWorkflowForRuns.value) {
    return;
  }

  const workflowId = selectedWorkflowForRuns.value.id;
  handleRunListOpenChange(false);
  void router.push({
    name: "workflow-editor",
    params: {
      id: workflowId,
    },
    query: {
      runId,
    },
  });
};

const useTemplate = (templateId: string) => {
  void router.push({
    name: "workflow-new",
    query: {
      template: templateId,
    },
  });
};
</script>
