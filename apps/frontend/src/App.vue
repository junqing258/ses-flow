<template>
  <RouterView />
  <AuthDialog />
  <Toaster position="top-right" rich-colors />
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import { RouterView } from "vue-router";

import AuthDialog from "@/components/AuthDialog.vue";
import { Toaster } from "@/components/ui/sonner";
import { useAuth } from "@/composables/useAuth";

const { initialize } = useAuth();

onMounted(() => {
  void initialize();
});
</script>
