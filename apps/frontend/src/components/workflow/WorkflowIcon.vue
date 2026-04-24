<script setup lang="ts">
import { computed, ref, watch } from "vue";

import {
  resolveWorkflowIcon,
  type WorkflowIconValue,
} from "@/features/workflow/model";

defineOptions({
  inheritAttrs: false,
});

const props = withDefaults(
  defineProps<{
    alt?: string;
    icon: WorkflowIconValue;
  }>(),
  {
    alt: "",
  },
);

const imageLoadFailed = ref(false);
const resolvedIcon = computed(() => resolveWorkflowIcon(props.icon));
const fallbackIcon = resolveWorkflowIcon("activity");
const fallbackIconComponent =
  fallbackIcon.kind === "component" ? fallbackIcon.component : undefined;
const shouldRenderImage = computed(
  () => resolvedIcon.value.kind === "image" && !imageLoadFailed.value,
);
const iconComponent = computed(() =>
  resolvedIcon.value.kind === "component"
    ? resolvedIcon.value.component
    : fallbackIconComponent,
);

watch(
  () => props.icon,
  () => {
    imageLoadFailed.value = false;
  },
);
</script>

<template>
  <img
    v-if="shouldRenderImage && resolvedIcon.kind === 'image'"
    v-bind="$attrs"
    :src="resolvedIcon.src"
    :alt="alt"
    class="object-contain"
    @error="imageLoadFailed = true"
  />
  <component
    :is="iconComponent"
    v-else
    v-bind="$attrs"
  />
</template>
