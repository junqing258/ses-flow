<template>
  <ElDialog
    :model-value="pluginAutoRegisterConfigDialogOpen"
    title="插件自动注册配置"
    append-to-body
    align-center
    class="settings-config-dialog"
    width="min(92vw, 40rem)"
    @update:model-value="handleDialogOpenChange"
  >
    <div class="space-y-5">
      <p class="text-sm leading-6 text-slate-600">
        保存后后端会持久化这些插件地址，并立即注册；服务重启后也会按这份配置恢复。
      </p>
      <div
        class="rounded-2xl border border-cyan-100 bg-cyan-50/80 px-4 py-3 text-sm leading-6 text-cyan-900"
      >
        每行填写一个插件服务 Base URL，例如
        <code class="rounded bg-white/80 px-1 py-0.5 text-[13px]"
          >http://127.0.0.1:6310</code
        >。
      </div>
      <div class="space-y-2">
        <label
          for="plugin-auto-register-base-urls"
          class="text-xs font-semibold tracking-[0.18em] text-slate-500 uppercase"
        >
          Plugin Base URLs
        </label>
        <textarea
          id="plugin-auto-register-base-urls"
          v-model="baseUrlsText"
          :disabled="isLoading || isSaving"
          rows="8"
          placeholder="http://127.0.0.1:6310&#10;http://127.0.0.1:6311"
          class="min-h-44 w-full rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm leading-6 text-slate-900 shadow-none outline-none transition focus:border-slate-300 focus:ring-2 focus:ring-slate-100 disabled:cursor-not-allowed disabled:opacity-60"
        />
        <p class="text-xs leading-5 text-slate-500">
          支持换行或逗号分隔；保存时会自动去重并清理首尾空格。
        </p>
      </div>
    </div>
    <template #footer>
      <ElButton
        native-type="button"
        :disabled="isLoading || isSaving"
        @click="handleReset"
      >
        清空配置
      </ElButton>
      <ElButton native-type="button" :disabled="isSaving" @click="handleCancel">
        取消
      </ElButton>
      <ElButton
        native-type="button"
        type="primary"
        :disabled="isLoading || isSaving"
        @click="handleSave"
      >
        {{ isSaving ? "保存中..." : "保存配置" }}
      </ElButton>
    </template>
  </ElDialog>
</template>
<script setup lang="ts">
import { ref, watch } from "vue";
import { toast } from "@/lib/element-toast";
import { usePluginAutoRegisterConfigDialog } from "@/composables/usePluginAutoRegisterConfigDialog";
import {
  fetchPluginAutoRegistrationConfig,
  updatePluginAutoRegistrationConfig,
} from "@/features/workflow/api";
const {
  pluginAutoRegisterConfigDialogOpen,
  setPluginAutoRegisterConfigDialogOpen,
} = usePluginAutoRegisterConfigDialog();
const baseUrlsText = ref("");
const isLoading = ref(false);
const isSaving = ref(false);
const normalizeBaseUrls = (value: string) => {
  const seen = new Set<string>();
  return value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0)
    .filter((item) => {
      if (seen.has(item)) {
        return false;
      }
      seen.add(item);
      return true;
    });
};
const validateBaseUrls = (baseUrls: string[]) => {
  for (const baseUrl of baseUrls) {
    try {
      const parsed = new URL(baseUrl);
      if (!["http:", "https:"].includes(parsed.protocol)) {
        throw new Error("unsupported protocol");
      }
    } catch {
      throw new Error(`插件地址格式不正确: ${baseUrl}`);
    }
  }
};
const loadConfig = async () => {
  isLoading.value = true;
  try {
    const config = await fetchPluginAutoRegistrationConfig();
    baseUrlsText.value = config.baseUrls.join("\n");
  } finally {
    isLoading.value = false;
  }
};
const handleDialogOpenChange = (open: boolean) => {
  setPluginAutoRegisterConfigDialogOpen(open);
};
const handleCancel = () => {
  setPluginAutoRegisterConfigDialogOpen(false);
};
const handleReset = async () => {
  isSaving.value = true;
  try {
    await updatePluginAutoRegistrationConfig({ baseUrls: [] });
    baseUrlsText.value = "";
    setPluginAutoRegisterConfigDialogOpen(false);
    toast.success("已清空插件自动注册配置");
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : "清空插件自动注册配置失败",
    );
  } finally {
    isSaving.value = false;
  }
};
const handleSave = async () => {
  const baseUrls = normalizeBaseUrls(baseUrlsText.value);
  try {
    validateBaseUrls(baseUrls);
  } catch (error) {
    toast.error(error instanceof Error ? error.message : "插件地址格式不正确");
    return;
  }
  isSaving.value = true;
  try {
    const response = await updatePluginAutoRegistrationConfig({ baseUrls });
    baseUrlsText.value = response.baseUrls.join("\n");
    setPluginAutoRegisterConfigDialogOpen(false);
    if (response.baseUrls.length === 0) {
      toast.success("已清空插件自动注册配置");
      return;
    }
    toast.success(
      `插件自动注册配置已保存，已注册 ${response.descriptors.length} 个插件节点`,
    );
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : "保存插件自动注册配置失败",
    );
  } finally {
    isSaving.value = false;
  }
};
watch(
  pluginAutoRegisterConfigDialogOpen,
  (open) => {
    if (!open) {
      return;
    }
    void loadConfig().catch((error) => {
      toast.error(
        error instanceof Error ? error.message : "获取插件自动注册配置失败",
      );
    });
  },
  {
    immediate: true,
  },
);
</script>
