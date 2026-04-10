<script setup lang="ts">
import type { TabsContentProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { TabsContent } from "reka-ui";

import { cn } from "@/lib/utils";

const props = defineProps<TabsContentProps & { class?: HTMLAttributes["class"] }>();
</script>

<template>
  <TabsContent
    v-bind="props"
    :class="
      cn(
        'mt-0 flex-1 outline-none data-[state=inactive]:hidden',
        props.class,
      )
    "
  >
    <slot />
  </TabsContent>
</template>
