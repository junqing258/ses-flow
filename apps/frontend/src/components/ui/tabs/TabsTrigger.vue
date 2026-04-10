<script setup lang="ts">
import type { TabsTriggerProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { TabsTrigger } from "reka-ui";

import { cn } from "@/lib/utils";

const props = defineProps<TabsTriggerProps & { class?: HTMLAttributes["class"] }>();
</script>

<template>
  <TabsTrigger
    v-bind="props"
    :class="
      cn(
        'inline-flex items-center justify-center rounded-t-lg border-b-2 border-transparent px-3 py-2 text-sm whitespace-nowrap transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#4f6af5]/30 disabled:pointer-events-none disabled:opacity-50 data-[state=active]:border-[#4f6af5] data-[state=active]:font-semibold data-[state=active]:text-[#4f6af5] data-[state=inactive]:text-slate-500 data-[state=inactive]:hover:text-slate-900',
        props.class,
      )
    "
  >
    <slot />
  </TabsTrigger>
</template>
