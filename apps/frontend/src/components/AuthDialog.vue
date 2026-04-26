<template>
  <ElDialog
    :model-value="dialogOpen"
    append-to-body
    align-center
    @update:model-value="setDialogOpen"
  >
    <div
      class="max-w-[min(92vw,38rem)] overflow-hidden border-white/10 bg-linear-to-br from-slate-950 via-slate-900 to-cyan-950 p-0 text-white shadow-[0_28px_80px_rgba(15,23,42,0.48)]"
    >
      <div class="relative overflow-hidden">
        <div class="absolute inset-0 bg-[radial-gradient(circle_at_top_right,rgba(34,211,238,0.18),transparent_36%),radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.14),transparent_32%)]" />
        <div class="relative grid gap-0 md:grid-cols-[0.9fr_1.1fr]">
          <aside class="border-b border-white/10 px-6 py-8 md:border-b-0 md:border-r">
            <p class="text-xs font-semibold tracking-[0.32em] text-cyan-300 uppercase">{{ t("auth.dialog.eyebrow") }}</p>
            <h2 class="mt-4 text-3xl font-semibold leading-tight text-white">
              {{ isAuthenticated ? t("auth.dialog.accountTitle") : currentTitle }}
            </h2>
            <p class="mt-3 text-sm leading-6 text-slate-300">
              {{ isAuthenticated ? t("auth.dialog.accountDescription") : currentDescription }}
            </p>
            <div class="mt-8 grid gap-3">
              <div class="rounded-2xl border border-white/10 bg-white/6 p-4">
                <p class="text-xs font-semibold tracking-[0.24em] text-cyan-200 uppercase">
                  {{ t("auth.dialog.featureOneTitle") }}
                </p>
                <p class="mt-2 text-sm leading-6 text-slate-300">{{ t("auth.dialog.featureOneDescription") }}</p>
              </div>
              <div class="rounded-2xl border border-white/10 bg-white/6 p-4">
                <p class="text-xs font-semibold tracking-[0.24em] text-cyan-200 uppercase">
                  {{ t("auth.dialog.featureTwoTitle") }}
                </p>
                <p class="mt-2 text-sm leading-6 text-slate-300">{{ t("auth.dialog.featureTwoDescription") }}</p>
              </div>
            </div>
          </aside>
          <div class="px-6 py-8">
            <div
              v-if="!isAuthenticated"
              class="inline-flex rounded-full border border-white/10 bg-white/8 p-1"
            >
              <button
                type="button"
                class="rounded-full px-4 py-2 text-sm font-medium transition"
                :class="dialogMode === 'login' ? 'bg-cyan-400 text-slate-950 shadow-sm' : 'text-slate-300 hover:text-white'"
                @click="switchMode('login')"
              >
                {{ t("auth.actions.login") }}
              </button>
              <button
                type="button"
                class="rounded-full px-4 py-2 text-sm font-medium transition"
                :class="dialogMode === 'register' ? 'bg-cyan-400 text-slate-950 shadow-sm' : 'text-slate-300 hover:text-white'"
                @click="switchMode('register')"
              >
                {{ t("auth.actions.register") }}
              </button>
            </div>
            <div
              v-if="isAuthenticated && user"
              class="mt-6 space-y-5 rounded-[24px] border border-white/10 bg-slate-950/45 p-5"
            >
              <div class="space-y-1">
                <p class="text-xs font-semibold tracking-[0.24em] text-cyan-200 uppercase">
                  {{ t("auth.account.displayName") }}
                </p>
                <p class="text-2xl font-semibold text-white">{{ user.displayName || t("auth.fallbackName") }}</p>
              </div>
              <div class="grid gap-4 sm:grid-cols-2">
                <div class="rounded-2xl border border-white/8 bg-white/4 p-4">
                  <p class="text-xs font-semibold tracking-[0.2em] text-slate-400 uppercase">
                    {{ t("auth.account.email") }}
                  </p>
                  <p class="mt-2 text-sm text-slate-100">{{ user.email }}</p>
                </div>
                <div class="rounded-2xl border border-white/8 bg-white/4 p-4">
                  <p class="text-xs font-semibold tracking-[0.2em] text-slate-400 uppercase">
                    {{ t("auth.account.role") }}
                  </p>
                  <p class="mt-2 text-sm text-slate-100">{{ t(`auth.roles.${user.role}`) }}</p>
                </div>
                <div class="rounded-2xl border border-white/8 bg-white/4 p-4">
                  <p class="text-xs font-semibold tracking-[0.2em] text-slate-400 uppercase">
                    {{ t("auth.account.expiresAt") }}
                  </p>
                  <p class="mt-2 text-sm text-slate-100">{{ sessionExpiryLabel }}</p>
                </div>
                <div class="rounded-2xl border border-white/8 bg-white/4 p-4">
                  <p class="text-xs font-semibold tracking-[0.2em] text-slate-400 uppercase">
                    {{ t("auth.account.lastLogin") }}
                  </p>
                  <p class="mt-2 text-sm text-slate-100">{{ lastLoginLabel }}</p>
                </div>
              </div>
              <div class="flex flex-wrap gap-3">
                <ElButton :disabled="isSubmitting" @click="handleLogout">
                  {{ isSubmitting ? t("auth.actions.processing") : t("auth.actions.logout") }}
                </ElButton>
                <ElButton @click="setDialogOpen(false)">
                  {{ t("auth.actions.close") }}
                </ElButton>
              </div>
            </div>
            <form v-else class="mt-6 space-y-5" @submit.prevent="handleSubmit">
              <div v-if="dialogMode === 'register'" class="space-y-2">
                <label class="text-sm font-medium text-slate-200" for="auth-display-name">
                  {{ t("auth.fields.displayName") }}
                </label>
                <ElInput
                  id="auth-display-name"
                  v-model="form.displayName"
                  :placeholder="t('auth.placeholders.displayName')"
                  class="h-11 border-white/10 bg-white/6 text-white placeholder:text-slate-400"
                />
              </div>
              <div class="space-y-2">
                <label class="text-sm font-medium text-slate-200" for="auth-email">
                  {{ t("auth.fields.email") }}
                </label>
                <ElInput
                  id="auth-email"
                  v-model="form.email"
                  type="email"
                  autocomplete="email"
                  :placeholder="t('auth.placeholders.email')"
                  class="h-11 border-white/10 bg-white/6 text-white placeholder:text-slate-400"
                />
              </div>
              <div class="space-y-2">
                <label class="text-sm font-medium text-slate-200" for="auth-password">
                  {{ t("auth.fields.password") }}
                </label>
                <ElInput
                  id="auth-password"
                  v-model="form.password"
                  type="password"
                  :autocomplete="dialogMode === 'login' ? 'current-password' : 'new-password'"
                  :placeholder="t('auth.placeholders.password')"
                  class="h-11 border-white/10 bg-white/6 text-white placeholder:text-slate-400"
                />
              </div>
              <div
                v-if="submitError"
                class="rounded-2xl border border-rose-300/20 bg-rose-400/10 px-4 py-3 text-sm text-rose-100"
              >
                {{ submitError }}
              </div>
              <div class="flex flex-wrap gap-3">
                <ElButton native-type="submit" :disabled="isSubmitting" class="min-w-28">
                  {{ isSubmitting ? t("auth.actions.processing") : submitLabel }}
                </ElButton>
                <ElButton native-type="button" :disabled="isSubmitting" @click="setDialogOpen(false)">
                  {{ t("auth.actions.cancel") }}
                </ElButton>
              </div>
            </form>
          </div>
        </div>
      </div>
    </div>
  </ElDialog>
</template>
<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@/lib/element-toast";
import { useAuth } from "@/composables/useAuth";
import type { AuthDialogMode } from "@/types/auth";
const { locale, t } = useI18n();
const {
  dialogMode,
  dialogOpen,
  isAuthenticated,
  isSubmitting,
  login,
  logout,
  register,
  session,
  setDialogMode,
  setDialogOpen,
  user,
} = useAuth();
const form = reactive({
  displayName: "",
  email: "",
  password: "",
});
const submitError = ref("");
const formatter = computed(
  () =>
    new Intl.DateTimeFormat(locale.value === "zh-CN" ? "zh-CN" : "en-US", {
      dateStyle: "medium",
      timeStyle: "short",
    }),
);
const currentTitle = computed(() =>
  dialogMode.value === "login" ? t("auth.dialog.loginTitle") : t("auth.dialog.registerTitle"),
);
const currentDescription = computed(() =>
  dialogMode.value === "login"
    ? t("auth.dialog.loginDescription")
    : t("auth.dialog.registerDescription"),
);
const submitLabel = computed(() =>
  dialogMode.value === "login" ? t("auth.actions.login") : t("auth.actions.register"),
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
const resetForm = () => {
  form.displayName = "";
  form.email = "";
  form.password = "";
  submitError.value = "";
};
const switchMode = (mode: AuthDialogMode) => {
  setDialogMode(mode);
  submitError.value = "";
};
const handleSubmit = async () => {
  submitError.value = "";
  try {
    if (dialogMode.value === "login") {
      await login({
        email: form.email,
        password: form.password,
      });
      toast.success(t("auth.feedback.loginSuccess"));
      resetForm();
      return;
    }
    await register({
      displayName: form.displayName,
      email: form.email,
      password: form.password,
    });
    toast.success(t("auth.feedback.registerSuccess"));
    resetForm();
  } catch (error) {
    submitError.value = error instanceof Error ? error.message : t("auth.feedback.genericError");
  }
};
const handleLogout = async () => {
  submitError.value = "";
  try {
    await logout();
    toast.success(t("auth.feedback.logoutSuccess"));
    setDialogOpen(false);
  } catch (error) {
    submitError.value = error instanceof Error ? error.message : t("auth.feedback.genericError");
  }
};
watch(dialogOpen, (open) => {
  if (!open) {
    resetForm();
  } else {
    submitError.value = "";
  }
});
watch(dialogMode, () => {
  submitError.value = "";
});
</script>
