<template>
  <ElDialog
    :model-value="aiProviderConfigDialogOpen"
    title="AI 供应商配置"
    append-to-body
    align-center
    class="settings-config-dialog"
    width="min(92vw, 34rem)"
    @update:model-value="handleDialogOpenChange"
  >
    <div class="space-y-5">
      <!-- <div class="rounded-2xl border border-cyan-100 bg-cyan-50/80 px-4 py-3 text-sm leading-6 text-cyan-900">
        当前配置只保存在本浏览器，不再回退 `.env`。请完整填写后再使用
        AI 编辑能力。
      </div> -->
      <div class="space-y-2">
        <label
          for="ai-provider-base-url"
          class="text-xs font-semibold tracking-[0.18em] text-slate-500 uppercase"
        >
          ANTHROPIC_BASE_URL
        </label>
        <ElInput
          id="ai-provider-base-url"
          v-model="form.baseUrl"
          placeholder="https://api.anthropic.com"
          class="h-11 rounded-xl border-slate-200 bg-slate-50 text-slate-900 shadow-none focus-visible:border-slate-300 focus-visible:ring-2 focus-visible:ring-slate-100"
        />
      </div>
      <div class="space-y-2">
        <label
          for="ai-provider-auth-token"
          class="text-xs font-semibold tracking-[0.18em] text-slate-500 uppercase"
        >
          ANTHROPIC_AUTH_TOKEN
        </label>
        <ElInput
          id="ai-provider-auth-token"
          v-model="form.authToken"
          type="password"
          placeholder="sk-ant-..."
          class="h-11 rounded-xl border-slate-200 bg-slate-50 text-slate-900 shadow-none focus-visible:border-slate-300 focus-visible:ring-2 focus-visible:ring-slate-100"
        />
      </div>
      <div class="space-y-2">
        <label
          for="ai-provider-model"
          class="text-xs font-semibold tracking-[0.18em] text-slate-500 uppercase"
        >
          ANTHROPIC_MODEL
        </label>
        <ElInput
          id="ai-provider-model"
          v-model="form.model"
          placeholder="claude-sonnet-4-6"
          class="h-11 rounded-xl border-slate-200 bg-slate-50 text-slate-900 shadow-none focus-visible:border-slate-300 focus-visible:ring-2 focus-visible:ring-slate-100"
        />
      </div>
    </div>
    <template #footer>
      <ElButton native-type="button" @click="handleReset">
        清空配置
      </ElButton>
      <ElButton native-type="button" @click="handleCancel">
        取消
      </ElButton>
      <ElButton native-type="button" type="primary" @click="handleSave">
        保存配置
      </ElButton>
    </template>
  </ElDialog>
</template>
<script setup lang="ts">
import { reactive, watch } from "vue";
import { toast } from "@/lib/element-toast";
import { useAiProviderConfig } from "@/composables/useAiProviderConfig";
import { useAiProviderConfigDialog } from "@/composables/useAiProviderConfigDialog";
import { isAiProviderConfigComplete } from "@/features/workflow/ai-provider";
const { aiProviderConfigDialogOpen, setAiProviderConfigDialogOpen } =
  useAiProviderConfigDialog();
const {
  aiProviderConfig,
  clearAiProviderConfig,
  reloadAiProviderConfig,
  saveAiProviderConfig,
} = useAiProviderConfig();
const form = reactive({
  baseUrl: "",
  authToken: "",
  model: "",
});
const hydrateForm = () => {
  form.baseUrl = aiProviderConfig.value?.baseUrl ?? "";
  form.authToken = aiProviderConfig.value?.authToken ?? "";
  form.model = aiProviderConfig.value?.model ?? "";
};
const handleDialogOpenChange = (open: boolean) => {
  setAiProviderConfigDialogOpen(open);
};
const handleCancel = () => {
  hydrateForm();
  setAiProviderConfigDialogOpen(false);
};
const handleReset = () => {
  clearAiProviderConfig();
  hydrateForm();
  setAiProviderConfigDialogOpen(false);
  toast.success("已清空 AI 供应商配置");
};
const handleSave = () => {
  const nextConfig = {
    baseUrl: form.baseUrl,
    authToken: form.authToken,
    model: form.model,
  };
  if (!isAiProviderConfigComplete(nextConfig)) {
    toast.error(
      "请完整填写 ANTHROPIC_BASE_URL、ANTHROPIC_AUTH_TOKEN、ANTHROPIC_MODEL",
    );
    return;
  }
  saveAiProviderConfig(nextConfig);
  hydrateForm();
  setAiProviderConfigDialogOpen(false);
  toast.success("AI 供应商配置已保存");
};
watch(
  aiProviderConfigDialogOpen,
  (open) => {
    if (!open) {
      return;
    }
    reloadAiProviderConfig();
    hydrateForm();
  },
  {
    immediate: true,
  },
);
</script>
