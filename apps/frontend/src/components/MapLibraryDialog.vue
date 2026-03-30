<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogScrollContent
      class="w-[min(1040px,96vw)] max-w-5xl gap-0 overflow-hidden border-slate-200 bg-slate-50 p-0 shadow-2xl"
    >
      <div class="border-b border-slate-200 bg-linear-to-r from-white via-slate-50 to-cyan-50/70 px-6 py-5">
        <DialogHeader class="space-y-2 pr-8 text-left">
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="space-y-2">
              <DialogTitle class="text-xl font-semibold tracking-[0.2px] text-slate-900">地图库</DialogTitle>
              <DialogDescription class="max-w-2xl text-sm leading-6 text-slate-600">
                集中管理已保存地图，支持按名称或标签搜索、按状态筛选，以及批量删除和快速打开。
              </DialogDescription>
            </div>
            <div class="flex flex-wrap gap-2 text-xs font-medium">
              <span class="ui-pill ui-pill-muted">共 {{ librarySummary.total }} 张地图</span>
              <span class="ui-pill ui-pill-info">已发布 {{ librarySummary.published }}</span>
              <span class="ui-pill ui-pill-warning">草稿 {{ librarySummary.draft }}</span>
            </div>
          </div>
        </DialogHeader>
      </div>

      <div
        class="grid gap-4 border-b border-slate-200 bg-white/80 px-6 py-4 lg:grid-cols-[minmax(0,1fr)_180px_auto] lg:items-end"
      >
        <div class="flex flex-col">
          <Label for="library-search" class="text-xs font-semibold tracking-[0.18em] text-slate-500 uppercase">
            搜索地图
          </Label>
          <div class="relative mt-2">
            <input
              id="library-search"
              v-model="search"
              class="input mt-0 h-10 pl-10 shadow-xs"
              placeholder="搜索名称、标签或场景"
            />
          </div>
        </div>

        <div class="flex flex-col">
          <Label for="library-filter" class="text-xs font-semibold tracking-[0.18em] text-slate-500 uppercase">
            状态筛选
          </Label>
          <select id="library-filter" v-model="filter" class="input mt-2 h-10">
            <option value="all">全部</option>
            <option value="published">已发布</option>
            <option value="draft">草稿</option>
          </select>
        </div>

        <div class="flex flex-wrap items-center gap-2 lg:self-end lg:justify-end">
          <Button size="sm" variant="outline" class="h-10" @click="emit('import')">导入</Button>
          <Button size="sm" variant="outline" class="h-10" @click="emit('update:open', false)">关闭</Button>
          <Button size="sm" variant="outline" class="h-10" :disabled="selectedCount === 0" @click="exportSelected">
            导出 JSONL<span v-if="selectedCount > 0">（{{ selectedCount }}）</span>
          </Button>
          <Button size="sm" variant="destructive" class="h-10" :disabled="selectedCount === 0" @click="deleteSelected">
            <Trash2 class="size-4" />
            删除选中<span v-if="selectedCount > 0">（{{ selectedCount }}）</span>
          </Button>
        </div>
      </div>

      <div class="flex flex-wrap items-center justify-between gap-2 border-b border-slate-200 bg-slate-50/80 px-6 py-3">
        <p class="m-0 text-sm text-slate-600">当前结果 {{ librarySummary.filtered }} / {{ librarySummary.total }}</p>
        <p class="m-0 text-sm text-slate-500">
          {{ selectedCount > 0 ? `已选择 ${selectedCount} 张地图` : "未选择任何地图" }}
        </p>
      </div>

      <div class="px-6 py-5">
        <div
          v-if="filteredItems.length === 0"
          class="grid min-h-65 place-items-center rounded-2xl border border-dashed border-slate-300 bg-white/85 p-8 text-center"
        >
          <div class="max-w-sm space-y-3">
            <div class="mx-auto grid size-12 place-items-center rounded-full bg-slate-100 text-slate-500">
              <Search class="size-5" />
            </div>
            <div class="space-y-1">
              <p class="m-0 text-base font-semibold text-slate-900">
                {{ librarySummary.total === 0 ? "地图库还是空的" : "没有匹配的地图" }}
              </p>
              <p class="m-0 text-sm leading-6 text-slate-500">
                {{
                  librarySummary.total === 0
                    ? "先把当前地图保存到地图库，之后就能在这里做统一管理。"
                    : "试试调整关键字或切换筛选条件，快速缩小结果范围。"
                }}
              </p>
            </div>
          </div>
        </div>

        <div v-else class="grid max-h-[56vh] gap-3 overflow-y-auto pr-1">
          <article
            v-for="item in filteredItems"
            :key="item.id"
            class="rounded-2xl border border-slate-200 bg-white shadow-sm transition-all duration-200 hover:border-slate-300 hover:shadow-md"
          >
            <div class="flex flex-col gap-4 p-4 lg:flex-row lg:items-center lg:justify-between">
              <div class="min-w-0 flex-1">
                <div class="flex items-start gap-3">
                  <Checkbox
                    :model-value="selectedIds.includes(item.id)"
                    class="mt-1 border-slate-300 data-[state=checked]:border-slate-900 data-[state=checked]:bg-slate-900"
                    @update:model-value="updateSelected(item.id, $event)"
                  />

                  <div class="min-w-0 space-y-3">
                    <div class="flex flex-wrap items-center gap-2">
                      <h4 class="truncate text-base font-semibold text-slate-900">{{ item.name }}</h4>
                      <span class="ui-pill" :class="item.draft ? 'ui-pill-warning' : 'ui-pill-info'">
                        {{ item.draft ? "草稿" : "已发布" }}
                      </span>
                      <span class="ui-pill ui-pill-muted">
                        {{ item.scene === "production" ? "生产" : "仿真" }}
                      </span>
                    </div>

                    <div class="flex flex-wrap gap-2 text-xs text-slate-500">
                      <span class="ui-pill ui-pill-muted">
                        {{ item.project.grid.width }} x {{ item.project.grid.height }}
                      </span>
                      <span class="ui-pill ui-pill-muted"> 设备 {{ item.project.devices.length }} </span>
                      <span class="ui-pill ui-pill-muted"> 路径 {{ item.project.overlays.robotPaths.length }} </span>
                      <span class="ui-pill ui-pill-muted">
                        面板 {{ item.project.overlays.platformPanels.length }}
                      </span>
                      <span class="ui-pill ui-pill-muted"> 更新于 {{ item.updatedAt }} </span>
                    </div>

                    <div class="flex flex-wrap gap-2">
                      <span v-for="tag in item.tags" :key="tag" class="ui-pill ui-pill-info"> #{{ tag }} </span>
                      <span v-if="item.tags.length === 0" class="py-1 text-xs text-slate-400">暂无标签</span>
                    </div>
                  </div>
                </div>
              </div>

              <div class="flex shrink-0 flex-wrap gap-2">
                <Button size="sm" variant="outline" class="min-w-20" @click="emit('export-item', item.id)">导出</Button>
                <Button size="sm" class="min-w-20" @click="emit('open-item', item.id)">打开</Button>
              </div>
            </div>
          </article>
        </div>
      </div>
    </DialogScrollContent>
  </Dialog>
</template>

<script setup lang="ts">
import { Search, Trash2 } from "lucide-vue-next";
import { computed, ref, watch } from "vue";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogDescription, DialogHeader, DialogScrollContent, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import type { MapLibraryItem } from "@/types/map";

type LibraryFilter = "all" | "published" | "draft";
type CheckedState = boolean | "indeterminate";

const props = defineProps<{
  open: boolean;
  items: MapLibraryItem[];
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  import: [];
  "open-item": [id: string];
  "export-item": [id: string];
  "export-selected": [ids: string[]];
  "delete-selected": [ids: string[]];
}>();

const search = ref("");
const filter = ref<LibraryFilter>("all");
const selectedIds = ref<string[]>([]);

const filteredItems = computed(() => {
  const keyword = search.value.trim().toLowerCase();

  return props.items.filter((item) => {
    if (filter.value === "published" && item.draft) {
      return false;
    }
    if (filter.value === "draft" && !item.draft) {
      return false;
    }
    if (!keyword) {
      return true;
    }

    return [item.name, item.scene === "production" ? "生产" : "仿真", ...item.tags].some((value) =>
      value.toLowerCase().includes(keyword),
    );
  });
});

const librarySummary = computed(() => {
  const total = props.items.length;
  const draft = props.items.filter((item) => item.draft).length;

  return {
    total,
    draft,
    published: total - draft,
    filtered: filteredItems.value.length,
  };
});

const selectedCount = computed(() => selectedIds.value.length);

const resetState = () => {
  search.value = "";
  filter.value = "all";
  selectedIds.value = [];
};

const updateSelected = (id: string, checked: CheckedState) => {
  if (checked === true) {
    if (!selectedIds.value.includes(id)) {
      selectedIds.value = [...selectedIds.value, id];
    }
    return;
  }

  selectedIds.value = selectedIds.value.filter((item) => item !== id);
};

const deleteSelected = () => {
  if (selectedIds.value.length === 0) {
    return;
  }

  emit("delete-selected", selectedIds.value);
  selectedIds.value = [];
};

const exportSelected = () => {
  if (selectedIds.value.length === 0) {
    return;
  }

  emit("export-selected", selectedIds.value);
};

watch(
  () => props.open,
  (open) => {
    if (open) {
      resetState();
    }
  },
);

watch(
  () => props.items,
  (items) => {
    const availableIds = new Set(items.map((item) => item.id));
    selectedIds.value = selectedIds.value.filter((id) => availableIds.has(id));
  },
  { deep: true },
);
</script>
