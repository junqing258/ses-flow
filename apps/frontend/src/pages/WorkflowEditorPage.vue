<template>
  <section class="flex h-screen flex-col overflow-hidden bg-[#1b1e2b] text-slate-100">
    <header class="border-b border-white/8 bg-[#1b1e2b]">
      <div class="flex h-14 items-center gap-4 px-4">
        <div class="flex min-w-[240px] items-center gap-3 pr-4">
          <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-[#4f6af5] shadow-[0_8px_18px_rgba(79,106,245,0.35)]">
            <span class="text-sm font-semibold text-white">S</span>
          </div>
          <div class="text-sm font-semibold tracking-[0.01em] text-white">SES Flow</div>
        </div>

        <div class="h-8 w-px bg-white/10" />

        <div class="flex min-w-0 flex-1 items-center gap-2 text-sm">
          <span class="text-slate-400">流程列表</span>
          <span class="text-slate-600">/</span>
          <span class="truncate font-medium text-white">sorting-main-flow</span>
          <span class="h-1.5 w-1.5 rounded-full bg-emerald-400" />
          <span class="rounded-full bg-white/10 px-2 py-1 text-xs text-slate-300">v3 草稿</span>
        </div>

        <div class="flex items-center gap-2">
          <Button variant="secondary" class="border border-white/6 bg-white/8 text-slate-200 hover:bg-white/12">
            保存草稿
          </Button>
          <Button variant="secondary" class="border border-white/6 bg-white/8 text-slate-200 hover:bg-white/12">
            校验
          </Button>
          <Button class="bg-[#4f6af5] text-white hover:bg-[#435ce0]" @click="handleExportJson">导出 JSON</Button>
          <Button variant="secondary" size="icon" class="border border-white/6 bg-white/8 text-slate-300 hover:bg-white/12">
            <MoreHorizontal class="h-4 w-4" />
          </Button>
        </div>
      </div>
    </header>

    <div class="grid min-h-0 flex-1 grid-cols-[280px_minmax(0,1fr)_340px] bg-[#f0f2f5]">
      <aside class="flex min-h-0 flex-col border-r border-slate-200 bg-white">
        <div class="border-b border-slate-200 p-3">
          <div class="relative">
            <Search class="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-slate-400" />
            <Input
              v-model="searchQuery"
              class="h-10 rounded-xl border-slate-200 bg-slate-50 pl-9 text-sm shadow-none focus-visible:ring-[#4f6af5]"
              placeholder="搜索节点..."
            />
          </div>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto pb-6">
          <div v-for="category in filteredCategories" :key="category.id" class="border-b border-slate-100 last:border-b-0">
            <button
              type="button"
              class="flex w-full items-center gap-2 bg-[#f3f5ff] px-3 py-2 text-left text-xs font-semibold tracking-[0.04em] text-slate-600"
              @click="toggleCategory(category.id)"
            >
              <component :is="resolveIcon(category.icon)" class="h-3.5 w-3.5 text-[#4f6af5]" />
              <span class="flex-1">{{ category.label }}</span>
              <component :is="isCategoryOpen(category.id) ? ChevronDown : ChevronRight" class="h-3.5 w-3.5 text-slate-400" />
            </button>

            <div v-if="isCategoryOpen(category.id)" class="py-1">
              <button
                v-for="item in category.items"
                :key="item.id"
                type="button"
                draggable="true"
                class="flex w-full cursor-grab items-center gap-3 px-3 py-2.5 text-left transition-colors active:cursor-grabbing"
                :class="selectedNodeData.kind === item.kind ? 'bg-slate-50' : 'hover:bg-slate-50'"
                @click="focusPaletteItem(item.kind)"
                @dragstart="handlePaletteDragStart($event, item.id)"
                @dragend="handlePaletteDragEnd"
              >
                <span class="h-6 w-1 rounded-full" :style="{ backgroundColor: item.accent }" />
                <div class="min-w-0 flex-1">
                  <p class="truncate text-[14px] font-medium text-slate-900">{{ item.label }}</p>
                </div>
                <GripVertical class="h-4 w-4 text-slate-400" />
              </button>
            </div>
          </div>

          <div
            v-if="!filteredCategories.length"
            class="flex min-h-[160px] items-center justify-center px-6 text-center text-sm leading-6 text-slate-400"
          >
            没有找到匹配的节点类型，换个关键词试试。
          </div>
        </div>
      </aside>

      <main
        class="workflow-canvas relative min-h-0 overflow-hidden"
        @dragenter.prevent="handleCanvasDragEnter"
        @dragover.prevent="handleCanvasDragOver"
        @drop.prevent="handleCanvasDrop"
      >
        <VueFlow
          :nodes="nodes"
          :edges="edges"
          fit-view-on-init
          class="h-full w-full"
          @connect="handleConnect"
          @node-click="handleNodeClick"
        >
          <template #node-workflow-card="nodeProps">
            <WorkflowCanvasNode v-bind="nodeProps" />
          </template>

          <template #node-terminal="nodeProps">
            <WorkflowTerminalNode v-bind="nodeProps" />
          </template>

          <template #node-branch-chip="nodeProps">
            <WorkflowBranchChipNode v-bind="nodeProps" />
          </template>

          <Background />
          <Controls />
          <MiniMap />
        </VueFlow>

        <div class="pointer-events-none absolute inset-0">
          <div
            v-if="isCanvasDropTarget"
            class="absolute inset-6 rounded-[28px] border-2 border-dashed border-[#4f6af5]/55 bg-[#4f6af5]/8 shadow-[inset_0_0_0_1px_rgba(79,106,245,0.08)]"
          >
            <div class="absolute inset-x-0 top-6 flex justify-center">
              <div class="rounded-full bg-white/92 px-4 py-2 text-xs font-semibold tracking-[0.03em] text-[#4f6af5] shadow-sm">
                松开鼠标，将节点放入画布
              </div>
            </div>
          </div>

          <div class="pointer-events-auto absolute left-4 top-4 flex items-center gap-1 rounded-xl border border-slate-200 bg-white/95 p-1 shadow-[0_12px_28px_rgba(15,23,42,0.08)] backdrop-blur">
            <button
              v-for="tool in canvasTools"
              :key="tool.id"
              type="button"
              class="flex h-8 w-8 items-center justify-center rounded-lg text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-900"
            >
              <component :is="resolveIcon(tool.icon)" class="h-4 w-4" />
            </button>
          </div>

          <div class="absolute bottom-6 left-1/2 -translate-x-1/2 rounded-full border border-amber-200 bg-amber-50 px-4 py-2 text-xs font-medium text-amber-700 shadow-sm">
            当前为草稿版本，发布后生效
          </div>
        </div>
      </main>

      <aside class="flex min-h-0 flex-col border-l border-slate-200 bg-white">
        <div class="flex h-16 items-center gap-3 border-b border-slate-200 px-4">
          <div
            class="flex h-10 w-10 items-center justify-center rounded-xl text-white shadow-[0_10px_22px_rgba(15,23,42,0.12)]"
            :style="{ backgroundColor: selectedNodeData.accent }"
          >
            <component :is="selectedNodeIcon" class="h-5 w-5" />
          </div>

          <div class="min-w-0 flex-1">
            <p class="truncate text-[15px] font-semibold text-slate-950">{{ selectedNodeData.subtitle ?? selectedNodeData.title }}</p>
            <p class="truncate text-xs text-slate-500">{{ selectedNodeData.title }} · {{ selectedNodeData.nodeKey }}</p>
          </div>

          <button
            type="button"
            class="flex h-8 w-8 items-center justify-center rounded-lg bg-slate-100 text-slate-500 transition-colors hover:bg-slate-200 hover:text-slate-900"
          >
            <MoreHorizontal class="h-4 w-4" />
          </button>
        </div>

        <Tabs
          class="min-h-0 flex-1"
          :model-value="activeTab"
          @update:model-value="handleTabChange"
        >
          <TabsList class="h-10">
            <TabsTrigger
              v-for="tab in visibleTabs"
              :key="tab"
              :value="tab"
            >
              {{ WORKFLOW_TAB_LABELS[tab] }}
            </TabsTrigger>
          </TabsList>

          <div class="min-h-0 flex-1 overflow-y-auto bg-[#f8f9fa] px-4 py-4">
            <TabsContent
              v-for="tab in visibleTabs"
              :key="tab"
              :value="tab"
              class="h-full"
            >
              <div v-if="getFieldsForTab(tab).length" class="space-y-4">
                <div v-for="field in getFieldsForTab(tab)" :key="`${tab}-${field.key}`" class="space-y-1.5">
                  <label class="block text-xs font-semibold tracking-[0.02em] text-slate-600">
                    {{ field.label }}
                  </label>

                  <Input
                    v-if="field.type === 'input'"
                    :model-value="field.value"
                    class="h-10 rounded-lg border-slate-300 bg-white shadow-none focus-visible:ring-[#4f6af5]"
                    @update:model-value="handleFieldUpdate(tab, field.key, String($event))"
                  />

                  <textarea
                    v-else-if="field.type === 'textarea'"
                    :value="field.value"
                    class="min-h-[96px] w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none transition focus:border-[#4f6af5] focus:ring-1 focus:ring-[#4f6af5]"
                    @input="handleFieldUpdate(tab, field.key, ($event.target as HTMLTextAreaElement).value)"
                  />

                  <div
                    v-else
                    class="flex min-h-10 items-center justify-between rounded-lg border px-3 text-sm"
                    :class="
                      field.type === 'readonly'
                        ? 'border-slate-200 bg-slate-100 text-slate-500'
                        : 'border-slate-300 bg-white text-slate-900'
                    "
                  >
                    <span>{{ field.value }}</span>
                    <ChevronDown v-if="field.type === 'select'" class="h-4 w-4 text-slate-400" />
                  </div>
                </div>
              </div>

              <div
                v-else
                class="flex min-h-[180px] items-center justify-center rounded-2xl border border-dashed border-slate-300 bg-white/75 px-6 text-center text-sm leading-6 text-slate-500"
              >
                {{ WORKFLOW_EMPTY_TAB_TEXT[tab] }}
              </div>
            </TabsContent>
          </div>
        </Tabs>

        <div class="flex h-14 items-center justify-end gap-2 border-t border-slate-200 px-4">
          <Button variant="secondary" class="bg-slate-100 text-slate-600 hover:bg-slate-200">取消</Button>
          <Button class="bg-[#4f6af5] text-white hover:bg-[#435ce0]">保存</Button>
        </div>
      </aside>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { Background } from "@vue-flow/background";
import { Controls } from "@vue-flow/controls";
import { type Connection, type Edge, VueFlow, useVueFlow } from "@vue-flow/core";
import { MiniMap } from "@vue-flow/minimap";
import { ChevronDown, ChevronRight, GripVertical, MoreHorizontal, Search } from "lucide-vue-next";
import { toast } from "vue-sonner";

import WorkflowBranchChipNode from "@/components/workflow/WorkflowBranchChipNode.vue";
import WorkflowCanvasNode from "@/components/workflow/WorkflowCanvasNode.vue";
import WorkflowTerminalNode from "@/components/workflow/WorkflowTerminalNode.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { createWorkflowExportDocument } from "@/features/workflow/export";
import {
  WORKFLOW_EMPTY_TAB_TEXT,
  WORKFLOW_ICON_MAP,
  WORKFLOW_PALETTE_CATEGORIES,
  WORKFLOW_TAB_LABELS,
  createWorkflowEdges,
  createWorkflowNodeDraft,
  createWorkflowNodes,
  createWorkflowPanels,
  type WorkflowFlowNode,
  type WorkflowIconKey,
  type WorkflowNodeData,
  type WorkflowNodeKind,
  type WorkflowNodePanel,
  type WorkflowPaletteItem,
  type WorkflowTabId,
} from "@/features/workflow/model";

const DRAG_DATA_TYPE = "application/x-ses-workflow-node";
const WORKFLOW_EDGE_STYLE = {
  stroke: "#CBD5E1",
  strokeWidth: 2,
};

const nodes = ref<WorkflowFlowNode[]>(createWorkflowNodes());
const edges = ref<Edge[]>(createWorkflowEdges());
const panelByNodeId = ref<Record<string, WorkflowNodePanel>>(createWorkflowPanels());
const searchQuery = ref("");
const selectedNodeId = ref("fetch_order");
const activeTab = ref<WorkflowTabId>("base");
const activeDragPaletteItemId = ref<string | null>(null);
const isCanvasDropTarget = ref(false);
const workflowMeta = {
  id: "sorting-main-flow",
  name: "sorting-main-flow",
  status: "draft" as const,
  version: "v3",
};
const expandedCategories = reactive<Record<string, boolean>>(
  Object.fromEntries(WORKFLOW_PALETTE_CATEGORIES.map((category) => [category.id, category.defaultOpen])),
);
const { screenToFlowCoordinate } = useVueFlow();

const canvasTools = [
  { id: "select", icon: "mousePointer" as WorkflowIconKey },
  { id: "pan", icon: "hand" as WorkflowIconKey },
  { id: "fit", icon: "maximize" as WorkflowIconKey },
  { id: "lock", icon: "lock" as WorkflowIconKey },
];

const EMPTY_NODE_DATA: WorkflowNodeData = {
  accent: "#3B82F6",
  icon: "database",
  kind: "fetch",
  nodeKey: "unselected",
  subtitle: "请选择节点",
  title: "未选择节点",
};

const selectedNodeData = ref<WorkflowNodeData>(EMPTY_NODE_DATA);
const selectedPanel = computed(() => panelByNodeId.value[selectedNodeId.value]);
const visibleTabs = computed(() => selectedPanel.value?.tabs ?? ["base"]);
const selectedNodeIcon = computed(() => WORKFLOW_ICON_MAP[selectedNodeData.value.icon]);

const filteredCategories = computed(() => {
  const keyword = searchQuery.value.trim().toLowerCase();

  return WORKFLOW_PALETTE_CATEGORIES.map((category) => ({
    ...category,
    items: keyword
      ? category.items.filter((item) => item.label.toLowerCase().includes(keyword))
      : category.items,
  })).filter((category) => category.items.length > 0);
});

const paletteItemMap = computed<Record<string, WorkflowPaletteItem>>(() =>
  WORKFLOW_PALETTE_CATEGORIES.flatMap((category) => category.items).reduce<Record<string, WorkflowPaletteItem>>((acc, item) => {
    acc[item.id] = item;
    return acc;
  }, {}),
);

watch(
  visibleTabs,
  (tabs) => {
    if (!tabs.includes(activeTab.value)) {
      activeTab.value = tabs[0];
    }
  },
  { immediate: true },
);

const resolveIcon = (icon: WorkflowIconKey) => WORKFLOW_ICON_MAP[icon];

const getFieldsForTab = (tab: WorkflowTabId) => selectedPanel.value?.fieldsByTab[tab] ?? [];

const handleTabChange = (value: string | number) => {
  if (typeof value === "string" && visibleTabs.value.includes(value as WorkflowTabId)) {
    activeTab.value = value as WorkflowTabId;
  }
};

const syncSelectedNodeData = () => {
  selectedNodeData.value = nodes.value.find((node) => node.id === selectedNodeId.value)?.data ?? EMPTY_NODE_DATA;
};

const setSelectedNode = (nodeId: string) => {
  selectedNodeId.value = nodeId;
  nodes.value = nodes.value.map((node) => ({
    ...node,
    data: {
      ...node.data,
      active: node.id === nodeId,
    },
  })) as WorkflowFlowNode[];
  syncSelectedNodeData();
};

const handleNodeClick = (payload: any) => {
  setSelectedNode(payload.node.id);
};

const getEdgeId = (connection: Connection) => {
  const sourceHandle = connection.sourceHandle ?? "default";
  const targetHandle = connection.targetHandle ?? "default";

  return `edge:${connection.source}:${sourceHandle}->${connection.target}:${targetHandle}`;
};

const handleConnect = (connection: Connection) => {
  if (!connection.source || !connection.target) {
    return;
  }

  const nextEdgeId = getEdgeId(connection);

  if (edges.value.some((edge) => edge.id === nextEdgeId)) {
    toast.info("这条连线已经存在");
    return;
  }

  edges.value = [
    ...edges.value,
    {
      id: nextEdgeId,
      source: connection.source,
      sourceHandle: connection.sourceHandle,
      target: connection.target,
      targetHandle: connection.targetHandle,
      type: "smoothstep",
      style: WORKFLOW_EDGE_STYLE,
    },
  ];

  toast.success("已创建连线");
};

const toggleCategory = (categoryId: string) => {
  expandedCategories[categoryId] = !expandedCategories[categoryId];
};

const isCategoryOpen = (categoryId: string) => {
  if (searchQuery.value.trim()) {
    return true;
  }

  return expandedCategories[categoryId];
};

const focusPaletteItem = (kind: WorkflowNodeKind) => {
  const targetNode = nodes.value.find((node) => node.data.kind === kind && node.type !== "branch-chip");

  if (targetNode) {
    setSelectedNode(targetNode.id);
  }
};

const handlePaletteDragStart = (event: DragEvent, itemId: string) => {
  if (!event.dataTransfer) {
    return;
  }

  activeDragPaletteItemId.value = itemId;
  event.dataTransfer.effectAllowed = "copy";
  event.dataTransfer.setData(DRAG_DATA_TYPE, itemId);
  event.dataTransfer.setData("text/plain", itemId);
};

const handlePaletteDragEnd = () => {
  activeDragPaletteItemId.value = null;
  isCanvasDropTarget.value = false;
};

const handleCanvasDragEnter = (event: DragEvent) => {
  if (!event.dataTransfer?.types.includes(DRAG_DATA_TYPE)) {
    return;
  }

  isCanvasDropTarget.value = true;
};

const handleCanvasDragOver = (event: DragEvent) => {
  if (!event.dataTransfer?.types.includes(DRAG_DATA_TYPE)) {
    return;
  }

  event.dataTransfer.dropEffect = "copy";
  isCanvasDropTarget.value = true;
};

const handleCanvasDrop = (event: DragEvent) => {
  isCanvasDropTarget.value = false;

  const itemId = event.dataTransfer?.getData(DRAG_DATA_TYPE);
  const item = itemId ? paletteItemMap.value[itemId] : undefined;

  activeDragPaletteItemId.value = null;

  if (!item) {
    return;
  }

  const flowPosition = screenToFlowCoordinate({
    x: event.clientX,
    y: event.clientY,
  });
  const { node, panel } = createWorkflowNodeDraft(
    item,
    {
      x: Math.max(24, flowPosition.x - (item.id === "palette-start" || item.id === "palette-end" ? 32 : 110)),
      y: Math.max(24, flowPosition.y - (item.id === "palette-start" || item.id === "palette-end" ? 32 : 36)),
    },
    nodes.value,
  );

  nodes.value = [...nodes.value, node];
  panelByNodeId.value = {
    ...panelByNodeId.value,
    [node.id]: panel,
  };
  setSelectedNode(node.id);

  toast.success(`已添加节点：${node.data.subtitle ?? node.data.title}`);
};

const handleFieldUpdate = (tab: WorkflowTabId, fieldKey: string, value: string) => {
  const panel = selectedPanel.value;

  if (!panel) {
    return;
  }

  const fields = panel.fieldsByTab[tab];
  const targetField = fields?.find((field) => field.key === fieldKey);

  if (!targetField) {
    return;
  }

  targetField.value = value;

  if (fieldKey === "nodeName") {
    nodes.value = nodes.value.map((node) =>
      node.id === selectedNodeId.value
        ? {
            ...node,
            data: {
              ...node.data,
              subtitle: value,
            },
          }
        : node,
    ) as WorkflowFlowNode[];
    syncSelectedNodeData();
  }
};

const handleExportJson = () => {
  try {
    const exportDocument = createWorkflowExportDocument(nodes.value, edges.value, panelByNodeId.value, {
      selectedNodeId: selectedNodeId.value,
      status: workflowMeta.status,
      version: workflowMeta.version,
      workflowId: workflowMeta.id,
      workflowName: workflowMeta.name,
    });
    const blob = new Blob([`${JSON.stringify(exportDocument, null, 2)}\n`], {
      type: "application/json;charset=utf-8",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");

    link.href = url;
    link.download = `${workflowMeta.name}.${workflowMeta.version}.json`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);

    toast.success("工作流 JSON 已导出");
  } catch (error) {
    toast.error(error instanceof Error ? error.message : "导出工作流 JSON 失败");
  }
};

setSelectedNode(selectedNodeId.value);
</script>

<style scoped>
.workflow-canvas :deep(.vue-flow__pane) {
  background:
    radial-gradient(circle at 20% 12%, rgba(255, 255, 255, 0.95), rgba(240, 242, 245, 0.92) 42%, rgba(226, 232, 240, 0.8) 100%),
    linear-gradient(180deg, rgba(79, 106, 245, 0.04), transparent 18%);
}

.workflow-canvas :deep(.vue-flow__edge-path) {
  stroke: #cbd5e1;
  stroke-width: 2;
}

.workflow-canvas :deep(.vue-flow__controls) {
  overflow: hidden;
  border: 1px solid #e2e8f0;
  border-radius: 14px;
  box-shadow: 0 16px 36px rgba(15, 23, 42, 0.08);
}

.workflow-canvas :deep(.vue-flow__controls-button) {
  display: flex;
  width: 34px;
  height: 34px;
  align-items: center;
  justify-content: center;
  border-bottom: 1px solid #e2e8f0;
  background: rgba(255, 255, 255, 0.96);
  color: #64748b;
}

.workflow-canvas :deep(.vue-flow__controls-button:hover) {
  background: #f8fafc;
  color: #0f172a;
}

.workflow-canvas :deep(.vue-flow__minimap) {
  border: 1px solid #e2e8f0;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.92);
  box-shadow: 0 18px 42px rgba(15, 23, 42, 0.08);
}

.workflow-canvas :deep(.vue-flow__background-pattern) {
  color: rgba(79, 106, 245, 0.08);
}
</style>
