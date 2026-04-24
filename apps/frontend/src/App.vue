<template>
  <RouterView />
  <AuthDialog />
  <AiProviderConfigDialog />
  <PluginAutoRegisterConfigDialog />
  <Toaster position="top-right" rich-colors />
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import { RouterView } from "vue-router";

import AiProviderConfigDialog from "@/components/AiProviderConfigDialog.vue";
import AuthDialog from "@/components/AuthDialog.vue";
import PluginAutoRegisterConfigDialog from "@/components/PluginAutoRegisterConfigDialog.vue";
import { Toaster } from "@/components/ui/sonner";
import { useAuth } from "@/composables/useAuth";

const { initialize } = useAuth();

onMounted(() => {
  void initialize();
});
</script>
