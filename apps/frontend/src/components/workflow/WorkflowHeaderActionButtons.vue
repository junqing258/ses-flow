<script setup lang="ts">
import { computed } from "vue";
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger,
} from "reka-ui";
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
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

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
const buttonVariant = computed(() => (isCompact.value ? "ghost" : "outline"));
const wrapperClass = computed(() =>
  isCompact.value ? "flex items-center gap-1.5" : "flex items-center gap-3",
);
const menuContentClass = computed(() =>
  cn(
    "z-50 min-w-52 overflow-hidden rounded-2xl border bg-white/96 p-1.5 text-slate-900 shadow-[0_18px_45px_rgba(15,23,42,0.14)] backdrop-blur outline-none",
    isCompact.value
      ? "border-[var(--panel-border)]"
      : "border-slate-200/80 ring-1 ring-white/70",
  ),
);
const menuItemClass = computed(() =>
  cn(
    "flex w-full cursor-pointer items-center gap-3 rounded-xl px-3 py-2.5 text-left text-sm font-medium outline-none transition-colors data-[highlighted]:bg-cyan-50 data-[highlighted]:text-cyan-700",
    isCompact.value ? "text-[var(--text)]" : "text-slate-700",
  ),
);
const moreButtonClass = computed(() =>
  isCompact.value
    ? "h-8 w-8 rounded-full text-[var(--app-muted)] hover:bg-[var(--app-primary-soft)]"
    : "h-10 rounded-full border-slate-200/80 bg-white/90 px-4 text-sm font-medium text-slate-700 shadow-[0_10px_30px_rgba(15,23,42,0.05)] hover:border-cyan-200 hover:bg-cyan-50 hover:text-cyan-700",
);
const settingsButtonClass = computed(() =>
  isCompact.value
    ? "h-8 w-8 rounded-full text-[var(--app-muted)] hover:bg-[var(--app-primary-soft)]"
    : "h-10 rounded-full border-slate-200/80 bg-white/90 px-4 text-sm font-medium text-slate-700 shadow-[0_10px_30px_rgba(15,23,42,0.05)] hover:border-cyan-200 hover:bg-cyan-50 hover:text-cyan-700",
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
    <DropdownMenuRoot>
      <DropdownMenuTrigger as-child>
        <Button
          :variant="buttonVariant"
          :size="isCompact ? 'icon' : 'default'"
          :class="moreButtonClass"
          :aria-label="isCompact ? '更多菜单' : undefined"
        >
          <MoreHorizontal class="h-4 w-4" />
          <span v-if="!isCompact">更多菜单</span>
        </Button>
      </DropdownMenuTrigger>

      <DropdownMenuPortal>
        <DropdownMenuContent
          align="end"
          :side-offset="8"
          :class="menuContentClass"
        >
          <DropdownMenuItem
            :class="menuItemClass"
            @select="openTroubleshootWorkbench"
          >
            <Search class="h-4 w-4 text-cyan-600" />
            <span>排障工作台</span>
          </DropdownMenuItem>
          <DropdownMenuItem :class="menuItemClass" @select="openHelp">
            <BookOpen class="h-4 w-4 text-amber-600" />
            <span>帮助文档</span>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenuPortal>
    </DropdownMenuRoot>

    <DropdownMenuRoot>
      <DropdownMenuTrigger as-child>
        <Button
          :variant="buttonVariant"
          :size="isCompact ? 'icon' : 'default'"
          :class="settingsButtonClass"
          :aria-label="isCompact ? '设置' : undefined"
        >
          <Settings class="h-4 w-4" />
          <span v-if="!isCompact">设置</span>
        </Button>
      </DropdownMenuTrigger>

      <DropdownMenuPortal>
        <DropdownMenuContent
          align="end"
          :side-offset="8"
          :class="menuContentClass"
        >
          <DropdownMenuItem
            :class="menuItemClass"
            @select="openAiProviderConfigDialog"
          >
            <Bot class="h-4 w-4 text-slate-700" />
            <span>AI 供应商配置</span>
          </DropdownMenuItem>
          <DropdownMenuItem
            :class="menuItemClass"
            @select="openPluginAutoRegisterConfigDialog"
          >
            <Plug class="h-4 w-4 text-cyan-700" />
            <span>插件自动注册</span>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenuPortal>
    </DropdownMenuRoot>
  </div>
</template>
