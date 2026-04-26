<template>
  <RouterView />
  <AuthDialog />
  <AiProviderConfigDialog />
  <PluginAutoRegisterConfigDialog />
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import { RouterView } from "vue-router";

import AiProviderConfigDialog from "@/components/AiProviderConfigDialog.vue";
import AuthDialog from "@/components/AuthDialog.vue";
import PluginAutoRegisterConfigDialog from "@/components/PluginAutoRegisterConfigDialog.vue";
import { useAuth } from "@/composables/useAuth";

const { initialize } = useAuth();

onMounted(() => {
  void initialize();
});
</script>
