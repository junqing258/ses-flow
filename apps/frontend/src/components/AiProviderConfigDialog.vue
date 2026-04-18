<template>
  <Dialog
    :open="aiProviderConfigDialogOpen"
    @update:open="handleDialogOpenChange"
  >
    <DialogContent
      class="max-w-[min(92vw,34rem)] rounded-[28px] border border-slate-200/80 bg-white p-0 shadow-[0_28px_80px_rgba(15,23,42,0.18)]"
    >
      <div class="overflow-hidden rounded-[28px]">
        <div class="border-b border-slate-200/80 bg-[linear-gradient(135deg,#f8fafc,#ecfeff)] px-6 py-5">
          <DialogHeader class="space-y-2">
            <DialogTitle class="text-xl font-semibold tracking-tight text-slate-950">
              AI 供应商配置
            </DialogTitle>
            <!-- <DialogDescription class="text-sm leading-6 text-slate-600">
              页面内每一次 AI 协作请求都会直接使用这里保存的用户配置。
            </DialogDescription> -->
          </DialogHeader>
        </div>

        <div class="space-y-5 px-6 py-6">
          <!-- <div class="rounded-2xl border border-cyan-100 bg-cyan-50/80 px-4 py-3 text-sm leading-6 text-cyan-900">
            当前配置只保存在本浏览器，不再回退 `.env`。请完整填写后再使用
            AI 编辑能力。
          </div> -->

          <div class="space-y-2">
            <Label for="ai-provider-base-url" class="text-xs font-semibold tracking-[0.18em] text-slate-500 uppercase">
              ANTHROPIC_BASE_URL
            </Label>
            <Input
              id="ai-provider-base-url"
              v-model="form.baseUrl"
              placeholder="https://api.anthropic.com"
              class="h-11 rounded-xl border-slate-200 bg-slate-50 text-slate-900 shadow-none focus-visible:border-slate-300 focus-visible:ring-2 focus-visible:ring-slate-100"
            />
          </div>

          <div class="space-y-2">
            <Label for="ai-provider-auth-token" class="text-xs font-semibold tracking-[0.18em] text-slate-500 uppercase">
              ANTHROPIC_AUTH_TOKEN
            </Label>
            <Input
              id="ai-provider-auth-token"
              v-model="form.authToken"
              type="password"
              placeholder="sk-ant-..."
              class="h-11 rounded-xl border-slate-200 bg-slate-50 text-slate-900 shadow-none focus-visible:border-slate-300 focus-visible:ring-2 focus-visible:ring-slate-100"
            />
          </div>

          <div class="space-y-2">
            <Label for="ai-provider-model" class="text-xs font-semibold tracking-[0.18em] text-slate-500 uppercase">
              ANTHROPIC_MODEL
            </Label>
            <Input
              id="ai-provider-model"
              v-model="form.model"
              placeholder="claude-sonnet-4-6"
              class="h-11 rounded-xl border-slate-200 bg-slate-50 text-slate-900 shadow-none focus-visible:border-slate-300 focus-visible:ring-2 focus-visible:ring-slate-100"
            />
          </div>
        </div>

        <DialogFooter class="border-t border-slate-200/80 bg-slate-50/80 px-6 py-4">
          <Button
            type="button"
            variant="outline"
            class="rounded-full border-slate-200 bg-white text-slate-700 hover:border-slate-300 hover:bg-slate-100"
            @click="handleReset"
          >
            清空配置
          </Button>
          <Button
            type="button"
            variant="outline"
            class="rounded-full border-slate-200 bg-white text-slate-700 hover:border-slate-300 hover:bg-slate-100"
            @click="handleCancel"
          >
            取消
          </Button>
          <Button
            type="button"
            class="rounded-full bg-slate-950 px-5 text-white hover:bg-slate-800"
            @click="handleSave"
          >
            保存配置
          </Button>
        </DialogFooter>
      </div>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { reactive } from "vue";
import { toast } from "vue-sonner";

import { useAiProviderConfigDialog } from "@/composables/useAiProviderConfigDialog";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  clearPersistedAiProviderConfig,
  isAiProviderConfigComplete,
  persistAiProviderConfig,
  readPersistedAiProviderConfig,
  resolveAiProviderConfigStorage,
} from "@/features/workflow/ai-provider";

const {
  aiProviderConfigDialogOpen,
  setAiProviderConfigDialogOpen,
} = useAiProviderConfigDialog();

const storage = resolveAiProviderConfigStorage();
const form = reactive({
  baseUrl: "",
  authToken: "",
  model: "",
});

const hydrateForm = () => {
  const persistedConfig = readPersistedAiProviderConfig(storage);

  form.baseUrl = persistedConfig?.baseUrl ?? "";
  form.authToken = persistedConfig?.authToken ?? "";
  form.model = persistedConfig?.model ?? "";
};

const handleDialogOpenChange = (open: boolean) => {
  if (open) {
    hydrateForm();
  }

  setAiProviderConfigDialogOpen(open);
};

const handleCancel = () => {
  hydrateForm();
  setAiProviderConfigDialogOpen(false);
};

const handleReset = () => {
  clearPersistedAiProviderConfig(storage);
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
    toast.error("请完整填写 ANTHROPIC_BASE_URL、ANTHROPIC_AUTH_TOKEN、ANTHROPIC_MODEL");
    return;
  }

  persistAiProviderConfig(storage, nextConfig);
  hydrateForm();
  setAiProviderConfigDialogOpen(false);
  toast.success("AI 供应商配置已保存");
};
</script>
