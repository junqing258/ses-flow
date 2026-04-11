<template>
  <section class="min-h-screen bg-linear-to-br from-slate-950 via-slate-900 to-cyan-950 px-6 py-16 text-white">
    <div class="mx-auto flex max-w-5xl flex-col gap-10">
      <div class="grid gap-6 lg:grid-cols-[1.15fr_0.85fr] lg:items-start">
        <div class="space-y-5">
          <p class="text-sm font-semibold tracking-[0.28em] text-cyan-300 uppercase">{{ t("about.eyebrow") }}</p>
          <div class="space-y-4">
            <h1 class="text-4xl font-semibold tracking-tight sm:text-5xl">{{ t("about.title") }}</h1>
            <p class="max-w-3xl text-base leading-7 text-slate-200 sm:text-lg">
              {{ t("about.description") }}
            </p>
          </div>
        </div>

        <aside
          class="rounded-[28px] border border-cyan-300/20 bg-linear-to-br from-white/12 via-cyan-300/8 to-slate-950/40 p-6 shadow-2xl backdrop-blur"
        >
          <div class="flex items-start justify-between gap-4">
            <div class="space-y-2">
              <p class="text-sm font-medium text-cyan-100">{{ t("about.authPanel.title") }}</p>
              <h2 class="text-2xl font-semibold text-white">
                {{ isAuthenticated ? t("about.authPanel.loggedInHeading") : t("about.authPanel.loggedOutHeading") }}
              </h2>
            </div>
            <span
              class="inline-flex items-center rounded-full border px-3 py-1 text-xs font-semibold"
              :class="
                isAuthenticated
                  ? 'border-emerald-300/30 bg-emerald-400/15 text-emerald-100'
                  : 'border-white/15 bg-white/8 text-slate-200'
              "
            >
              {{ isAuthenticated ? t("auth.status.authenticated") : t("auth.status.guest") }}
            </span>
          </div>

          <p class="mt-4 text-sm leading-6 text-slate-300">
            {{ isAuthenticated ? t("about.authPanel.loggedInDescription") : t("about.authPanel.loggedOutDescription") }}
          </p>

          <div class="mt-6 rounded-2xl border border-white/10 bg-slate-950/45 p-4">
            <template v-if="isAuthenticated && user">
              <div class="space-y-3">
                <div>
                  <p class="text-xs font-semibold tracking-[0.24em] text-cyan-200 uppercase">
                    {{ t("auth.account.displayName") }}
                  </p>
                  <p class="mt-1 text-lg font-semibold text-white">{{ user.displayName || t("auth.fallbackName") }}</p>
                </div>
                <div class="grid gap-3 sm:grid-cols-2">
                  <div>
                    <p class="text-xs font-semibold tracking-[0.24em] text-slate-400 uppercase">
                      {{ t("auth.account.email") }}
                    </p>
                    <p class="mt-1 text-sm text-slate-100">{{ user.email }}</p>
                  </div>
                  <div>
                    <p class="text-xs font-semibold tracking-[0.24em] text-slate-400 uppercase">
                      {{ t("auth.account.role") }}
                    </p>
                    <p class="mt-1 text-sm text-slate-100">{{ roleLabel }}</p>
                  </div>
                  <div>
                    <p class="text-xs font-semibold tracking-[0.24em] text-slate-400 uppercase">
                      {{ t("auth.account.expiresAt") }}
                    </p>
                    <p class="mt-1 text-sm text-slate-100">{{ sessionExpiryLabel }}</p>
                  </div>
                  <div>
                    <p class="text-xs font-semibold tracking-[0.24em] text-slate-400 uppercase">
                      {{ t("auth.account.lastLogin") }}
                    </p>
                    <p class="mt-1 text-sm text-slate-100">{{ lastLoginLabel }}</p>
                  </div>
                </div>
              </div>
            </template>

            <template v-else>
              <div class="space-y-3">
                <p class="text-sm font-medium text-white">{{ t("about.authPanel.benefitsTitle") }}</p>
                <ul class="space-y-2 text-sm leading-6 text-slate-300">
                  <li>{{ t("about.authPanel.benefits.one") }}</li>
                  <li>{{ t("about.authPanel.benefits.two") }}</li>
                  <li>{{ t("about.authPanel.benefits.three") }}</li>
                </ul>
              </div>
            </template>
          </div>

          <div class="mt-6 flex flex-wrap gap-3">
            <Button v-if="!isAuthenticated" @click="openAuthDialog('login')">
              {{ t("auth.actions.login") }}
            </Button>
            <Button v-if="!isAuthenticated" variant="outline" @click="openAuthDialog('register')">
              {{ t("auth.actions.register") }}
            </Button>
            <Button v-if="isAuthenticated" @click="openAuthDialog('login')">
              {{ t("auth.actions.manageAccount") }}
            </Button>
            <Button
              v-if="isAuthenticated"
              variant="outline"
              :disabled="isSubmitting"
              @click="handleLogout"
            >
              {{ isSubmitting ? t("auth.actions.processing") : t("auth.actions.logout") }}
            </Button>
          </div>
        </aside>
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
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import { useAuth } from "@/composables/useAuth";
import { LOCALE_OPTIONS, setLocale, type AppLocale } from "@/i18n";

const { locale, t } = useI18n();
const { isAuthenticated, isSubmitting, logout, openAuthDialog, session, user } = useAuth();

const currentLocaleLabel = computed(
  () => LOCALE_OPTIONS.find((option) => option.value === locale.value)?.label ?? locale.value,
);

const formatter = computed(
  () =>
    new Intl.DateTimeFormat(locale.value === "zh-CN" ? "zh-CN" : "en-US", {
      dateStyle: "medium",
      timeStyle: "short",
    }),
);

const sessionExpiryLabel = computed(() => {
  if (!session.value?.expiresAt) {
    return t("auth.account.unavailable");
  }

  return formatter.value.format(new Date(session.value.expiresAt));
});

const lastLoginLabel = computed(() => {
  if (!user.value?.lastLoginAt) {
    return t("auth.account.unavailable");
  }

  return formatter.value.format(new Date(user.value.lastLoginAt));
});

const roleLabel = computed(() => {
  if (!user.value?.role) {
    return t("auth.account.unavailable");
  }

  return t(`auth.roles.${user.value.role}`);
});

const handleLocaleChange = (value: AppLocale) => {
  setLocale(value);
};

const handleLogout = async () => {
  try {
    await logout();
    toast.success(t("auth.feedback.logoutSuccess"));
  } catch (error) {
    toast.error(error instanceof Error ? error.message : t("auth.feedback.genericError"));
  }
};
</script>
