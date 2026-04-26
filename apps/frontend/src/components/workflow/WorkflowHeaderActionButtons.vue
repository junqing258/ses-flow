<script setup lang="ts">
import { computed } from "vue";
import {
  BookOpen,
  Bot,
  MoreHorizontal,
  Plug,
  Search,
  Settings,
} from "lucide-vue-next";
import { type RouteLocationRaw, useRouter } from "vue-router";
import { useAiProviderConfigDialog } from "@/composables/useAiProviderConfigDialog";
import { usePluginAutoRegisterConfigDialog } from "@/composables/usePluginAutoRegisterConfigDialog";
interface Props {
  appearance?: "compact" | "pill";
}
const props = withDefaults(defineProps<Props>(), {
  appearance: "compact",
});
const router = useRouter();
const { openAiProviderConfigDialog } = useAiProviderConfigDialog();
const { openPluginAutoRegisterConfigDialog } =
  usePluginAutoRegisterConfigDialog();
const isCompact = computed(() => props.appearance === "compact");
const wrapperClass = computed(() =>
  isCompact.value ? "flex items-center gap-1.5" : "flex items-center gap-3",
);
const moreButtonClass = computed(() =>
  isCompact.value ? "" : "shadow-[0_10px_30px_rgba(15,23,42,0.05)]",
);
const settingsButtonClass = computed(() =>
  isCompact.value ? "" : "shadow-[0_10px_30px_rgba(15,23,42,0.05)]",
);
const openRouteInNewTab = (location: RouteLocationRaw) => {
  const resolvedRoute = router.resolve(location);
  window.open(resolvedRoute.href, "_blank", "noopener,noreferrer");
};
const openTroubleshootWorkbench = () => {
  openRouteInNewTab({ name: "troubleshoot-workbench" });
};
const openHelp = () => {
  openRouteInNewTab({ path: "/help" });
};
</script>
<template>
  <div :class="wrapperClass">
    <ElDropdown trigger="click" placement="bottom-end">
      <span>
        <ElButton
          :text="isCompact"
          :plain="!isCompact"
          :circle="isCompact"
          :class="moreButtonClass"
          :aria-label="isCompact ? '更多菜单' : undefined"
        >
          <MoreHorizontal class="h-4 w-4" />
          <span v-if="!isCompact">更多菜单</span>
        </ElButton>
      </span>
      <template #dropdown>
        <ElDropdownMenu>
          <ElDropdownItem @click="openTroubleshootWorkbench">
            <Search class="mr-2 h-4 w-4 text-cyan-600" />
            排障工作台
          </ElDropdownItem>
          <ElDropdownItem @click="openHelp">
            <BookOpen class="mr-2 h-4 w-4 text-amber-600" />
            帮助文档
          </ElDropdownItem>
        </ElDropdownMenu>
      </template>
    </ElDropdown>
    <ElDropdown trigger="click" placement="bottom-end">
      <span>
        <ElButton
          :text="isCompact"
          :plain="!isCompact"
          :circle="isCompact"
          :class="settingsButtonClass"
          :aria-label="isCompact ? '设置' : undefined"
        >
          <Settings class="h-4 w-4" />
          <span v-if="!isCompact">设置</span>
        </ElButton>
      </span>
      <template #dropdown>
        <ElDropdownMenu>
          <ElDropdownItem @click="openAiProviderConfigDialog">
            <Bot class="mr-2 h-4 w-4 text-slate-700" />
            AI 供应商配置
          </ElDropdownItem>
          <ElDropdownItem @click="openPluginAutoRegisterConfigDialog">
            <Plug class="mr-2 h-4 w-4 text-cyan-700" />
            插件自动注册
          </ElDropdownItem>
        </ElDropdownMenu>
      </template>
    </ElDropdown>
  </div>
</template>
