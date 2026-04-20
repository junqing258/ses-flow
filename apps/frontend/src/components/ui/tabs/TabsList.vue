<script setup lang="ts">
import type { TabsListProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { TabsList } from "reka-ui";

import { cn } from "@/lib/utils";

const props = defineProps<TabsListProps & { class?: HTMLAttributes["class"] }>();
</script>

<template>
  <TabsList
    :class="
      cn(
        'inline-flex h-10 items-end gap-1 rounded-none border-b border-[var(--panel-border)] bg-transparent px-2 text-[var(--app-muted)]',
        props.class,
      )
    "
  >
    <slot />
  </TabsList>
</template>
