<route>
{
  name: "login",
  meta: {
    public: true
  }
}
</route>

<template>
  <main class="min-h-screen bg-slate-100 text-slate-950">
    <section class="grid min-h-screen lg:grid-cols-[1fr_460px]">
      <div class="flex min-h-[42vh] flex-col justify-between bg-white px-6 py-8 sm:px-10 lg:min-h-screen lg:px-14">
        <div class="flex items-center justify-between gap-4">
          <div>
            <p class="text-sm font-semibold text-cyan-700">{{ t("auth.loginPage.product") }}</p>
            <h1 class="mt-2 text-3xl font-semibold text-slate-950 sm:text-4xl">
              {{ t("auth.loginPage.title") }}
            </h1>
          </div>
        </div>

        <div class="my-12 max-w-3xl">
          <div class="grid gap-4 sm:grid-cols-3">
            <div class="border-l-4 border-cyan-500 bg-slate-50 px-4 py-3">
              <p class="text-xs font-semibold uppercase text-slate-500">{{ t("auth.loginPage.panelOneLabel") }}</p>
              <p class="mt-2 text-sm font-medium text-slate-900">{{ t("auth.loginPage.panelOneText") }}</p>
            </div>
            <div class="border-l-4 border-emerald-500 bg-slate-50 px-4 py-3">
              <p class="text-xs font-semibold uppercase text-slate-500">{{ t("auth.loginPage.panelTwoLabel") }}</p>
              <p class="mt-2 text-sm font-medium text-slate-900">{{ t("auth.loginPage.panelTwoText") }}</p>
            </div>
            <div class="border-l-4 border-amber-500 bg-slate-50 px-4 py-3">
              <p class="text-xs font-semibold uppercase text-slate-500">{{ t("auth.loginPage.panelThreeLabel") }}</p>
              <p class="mt-2 text-sm font-medium text-slate-900">{{ t("auth.loginPage.panelThreeText") }}</p>
            </div>
          </div>
        </div>

        <p class="text-sm text-slate-500">{{ t("auth.loginPage.footer") }}</p>
      </div>

      <aside class="flex items-center bg-slate-950 px-6 py-8 sm:px-10">
        <div class="w-full">
          <div class="mb-8">
            <p class="text-sm font-semibold text-cyan-300">{{ t("auth.actions.login") }}</p>
            <h2 class="mt-2 text-2xl font-semibold text-white">{{ t("auth.dialog.loginTitle") }}</h2>
            <p class="mt-3 text-sm leading-6 text-slate-300">{{ t("auth.dialog.loginDescription") }}</p>
          </div>

          <form class="space-y-5" @submit.prevent="handleSubmit">
            <div class="space-y-2">
              <label class="text-sm font-medium text-slate-200" for="login-username">
                {{ t("auth.fields.username") }}
              </label>
              <ElInput
                id="login-username"
                v-model="form.username"
                autocomplete="username"
                :placeholder="t('auth.placeholders.username')"
                size="large"
              />
            </div>

            <div class="space-y-2">
              <label class="text-sm font-medium text-slate-200" for="login-password">
                {{ t("auth.fields.password") }}
              </label>
              <ElInput
                id="login-password"
                v-model="form.password"
                type="password"
                autocomplete="current-password"
                :placeholder="t('auth.placeholders.password')"
                show-password
                size="large"
              />
            </div>

            <div
              v-if="submitError"
              class="border border-rose-300/25 bg-rose-400/10 px-4 py-3 text-sm text-rose-100"
            >
              {{ submitError }}
            </div>

            <ElButton native-type="submit" type="primary" size="large" class="w-full" :loading="isSubmitting">
              {{ isSubmitting ? t("auth.actions.processing") : t("auth.actions.login") }}
            </ElButton>
          </form>
        </div>
      </aside>
    </section>
  </main>
</template>

<script setup lang="ts">
import { reactive, ref } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { toast } from "@/lib/element-toast";
import { useAuth } from "@/composables/useAuth";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const { isSubmitting, login } = useAuth();

const form = reactive({
  username: "",
  password: "",
});
const submitError = ref("");

const handleSubmit = async () => {
  submitError.value = "";
  try {
    await login({
      login: form.username,
      password: form.password,
    });
    toast.success(t("auth.feedback.loginSuccess"));
    const redirect = typeof route.query.redirect === "string" ? route.query.redirect : "/";
    await router.replace(redirect);
  } catch (error) {
    submitError.value = error instanceof Error ? error.message : t("auth.feedback.genericError");
  }
};
</script>
