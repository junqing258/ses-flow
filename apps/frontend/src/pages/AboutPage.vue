<template>
  <section class="min-h-screen bg-linear-to-br from-slate-950 via-slate-900 to-cyan-950 px-6 py-16 text-white">
    <div class="mx-auto flex max-w-5xl flex-col gap-10">
      <div class="space-y-5">
        <p class="text-sm font-semibold tracking-[0.28em] text-cyan-300 uppercase">{{ t("about.eyebrow") }}</p>
        <div class="space-y-4">
          <h1 class="text-4xl font-semibold tracking-tight sm:text-5xl">{{ t("about.title") }}</h1>
          <p class="max-w-3xl text-base leading-7 text-slate-200 sm:text-lg">
            {{ t("about.description") }}
          </p>
        </div>
      </div>

      <div class="grid gap-6 rounded-3xl border border-white/10 bg-white/6 p-6 shadow-2xl backdrop-blur lg:grid-cols-[1.2fr_0.8fr]">
        <div class="grid gap-4 md:grid-cols-3">
          <article class="rounded-2xl border border-white/10 bg-slate-950/50 p-5">
            <h2 class="text-lg font-semibold text-white">{{ t("about.cards.detection.title") }}</h2>
            <p class="mt-2 text-sm leading-6 text-slate-300">{{ t("about.cards.detection.description") }}</p>
          </article>
          <article class="rounded-2xl border border-white/10 bg-slate-950/50 p-5">
            <h2 class="text-lg font-semibold text-white">{{ t("about.cards.persistence.title") }}</h2>
            <p class="mt-2 text-sm leading-6 text-slate-300">{{ t("about.cards.persistence.description") }}</p>
          </article>
          <article class="rounded-2xl border border-white/10 bg-slate-950/50 p-5">
            <h2 class="text-lg font-semibold text-white">{{ t("about.cards.runtime.title") }}</h2>
            <p class="mt-2 text-sm leading-6 text-slate-300">{{ t("about.cards.runtime.description") }}</p>
          </article>
        </div>

        <aside class="rounded-2xl border border-cyan-300/20 bg-cyan-400/10 p-6">
          <p class="text-sm font-medium text-cyan-200">{{ t("about.currentLocale") }}</p>
          <p class="mt-2 text-2xl font-semibold text-white">{{ currentLocaleLabel }}</p>

          <div class="mt-6">
            <p class="text-sm font-medium text-cyan-100">{{ t("about.switcherLabel") }}</p>
            <div class="mt-3 flex flex-wrap gap-3">
              <Button
                v-for="option in LOCALE_OPTIONS"
                :key="option.value"
                size="sm"
                :variant="locale === option.value ? 'default' : 'outline'"
                @click="handleLocaleChange(option.value)"
              >
                {{ option.label }}
              </Button>
            </div>
          </div>
        </aside>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import { LOCALE_OPTIONS, setLocale, type AppLocale } from "@/i18n";

const { locale, t } = useI18n();

const currentLocaleLabel = computed(
  () => LOCALE_OPTIONS.find((option) => option.value === locale.value)?.label ?? locale.value,
);

const handleLocaleChange = (value: AppLocale) => {
  setLocale(value);
};
</script>
