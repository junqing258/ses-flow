<route>
{
  name: "login",
  meta: {
    public: true
  }
}
</route>

<template>
  <main class="relative min-h-screen w-full overflow-hidden bg-black flex items-center justify-center font-sans">
    <!-- Background Layer -->
    <div class="absolute inset-0 z-0 flex">
      <!-- Deep cyan glow on the left/bottom -->
      <div class="absolute bottom-[-10%] left-[-5%] w-[45%] h-[45%] rounded-full bg-cyan-600/20 blur-[120px] pointer-events-none"></div>
      <div class="absolute top-[15%] left-[-10%] w-[35%] h-[35%] rounded-full bg-cyan-900/20 blur-[100px] pointer-events-none"></div>
      
      <!-- Right side grid with cyan nodes -->
      <div class="absolute inset-y-0 right-0 w-[60%] opacity-60 mask-gradient-left pointer-events-none">
        <svg class="absolute inset-0 h-full w-full" xmlns="http://www.w3.org/2000/svg">
          <defs>
            <pattern id="hyperleap-grid" width="64" height="64" patternUnits="userSpaceOnUse">
              <path d="M 64 0 L 0 0 0 64" fill="none" stroke="rgba(255,255,255,0.06)" stroke-width="1" />
              <circle cx="0" cy="0" r="2" fill="#06b6d4" class="opacity-80" />
            </pattern>
          </defs>
          <rect width="100%" height="100%" fill="url(#hyperleap-grid)" />
        </svg>
      </div>
    </div>

    <!-- Main Container -->
    <div class="relative z-10 w-full max-w-7xl p-6 lg:p-12">
      <div class="grid lg:grid-cols-[1.2fr_1fr] gap-12 lg:gap-24 items-center">
        
        <!-- Left Side: Information (Text-focused, no heavy cards) -->
        <div class="flex flex-col justify-center h-full text-white">
          <div class="mb-8 inline-flex items-center">
            <div class="h-1 w-6 bg-cyan-400 mr-4 shadow-[0_0_8px_rgba(6,182,212,0.8)]"></div>
            <span class="text-sm font-semibold tracking-widest text-cyan-400 uppercase">{{ t("auth.loginPage.product") }}</span>
          </div>
          
          <h1 class="text-5xl lg:text-6xl font-bold tracking-tight text-white mb-6 leading-tight">
            {{ t("auth.loginPage.title") }}
          </h1>
          
          <div class="mt-16 grid gap-8 sm:grid-cols-2 lg:grid-cols-1 xl:grid-cols-3">
            <!-- Feature 1 -->
            <div class="relative">
              <div class="mb-4 inline-flex h-10 w-10 items-center justify-center rounded-lg bg-cyan-500/10 border border-cyan-500/20 text-cyan-400">
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>
              </div>
              <p class="text-sm font-semibold text-white mb-2">{{ t("auth.loginPage.panelOneLabel") }}</p>
              <p class="text-sm text-slate-400 leading-relaxed">{{ t("auth.loginPage.panelOneText") }}</p>
            </div>

            <!-- Feature 2 -->
            <div class="relative">
              <div class="mb-4 inline-flex h-10 w-10 items-center justify-center rounded-lg bg-cyan-500/10 border border-cyan-500/20 text-cyan-400">
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline></svg>
              </div>
              <p class="text-sm font-semibold text-white mb-2">{{ t("auth.loginPage.panelTwoLabel") }}</p>
              <p class="text-sm text-slate-400 leading-relaxed">{{ t("auth.loginPage.panelTwoText") }}</p>
            </div>

            <!-- Feature 3 -->
            <div class="relative">
              <div class="mb-4 inline-flex h-10 w-10 items-center justify-center rounded-lg bg-cyan-500/10 border border-cyan-500/20 text-cyan-400">
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
              </div>
              <p class="text-sm font-semibold text-white mb-2">{{ t("auth.loginPage.panelThreeLabel") }}</p>
              <p class="text-sm text-slate-400 leading-relaxed">{{ t("auth.loginPage.panelThreeText") }}</p>
            </div>
          </div>
          
          <div class="mt-16 flex items-center gap-4">
            <p class="text-xs font-medium text-slate-500 uppercase tracking-widest">{{ t("auth.loginPage.footer") }}</p>
          </div>
        </div>

        <!-- Right Side: Login Form -->
        <div class="relative">
          <!-- Ambient glow behind the card -->
          <div class="absolute -inset-1 rounded-[2rem] bg-cyan-500/5 blur-lg"></div>
          
          <div class="relative bg-black/60 backdrop-blur-2xl p-10 lg:p-14 rounded-3xl border border-white/10 shadow-xl">
            <div class="mb-10 text-center">
              <h2 class="text-3xl font-bold text-white tracking-tight">{{ t("auth.dialog.loginTitle") }}</h2>
              <!-- <p class="mt-3 text-sm text-slate-400">{{ t("auth.dialog.loginDescription") }}</p> -->
            </div>

            <form class="space-y-6" @submit.prevent="handleSubmit">
              <div class="space-y-2.5">
                <label class="text-xs font-semibold text-slate-400 uppercase tracking-wider" for="login-username">
                  {{ t("auth.fields.username") }}
                </label>
                <ElInput
                  id="login-username"
                  v-model="form.username"
                  autocomplete="username"
                  :placeholder="t('auth.placeholders.username')"
                  size="large"
                  class="!h-12 w-full custom-input"
                />
              </div>

              <div class="space-y-2.5">
                <label class="text-xs font-semibold text-slate-400 uppercase tracking-wider" for="login-password">
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
                  class="!h-12 w-full custom-input"
                />
              </div>

              <div
                v-if="submitError"
                class="flex items-center gap-3 rounded-xl border border-rose-500/20 bg-rose-500/10 px-4 py-3 text-sm text-rose-400 backdrop-blur-md"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="flex-shrink-0"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>
                {{ submitError }}
              </div>

              <ElButton 
                native-type="submit" 
                type="primary" 
                size="large" 
                class="w-full !h-12 !mt-8 !rounded-xl !text-sm !font-bold !uppercase !tracking-wider !bg-cyan-500 !border-cyan-500 hover:!bg-cyan-400 hover:!border-cyan-400 !text-black !shadow-[0_0_10px_rgba(6,182,212,0.2)] transition-all hover:!shadow-[0_0_15px_rgba(6,182,212,0.4)]" 
                :loading="isSubmitting"
              >
                {{ isSubmitting ? t("auth.actions.processing") : t("auth.actions.login") }}
              </ElButton>
            </form>
          </div>
        </div>
      </div>
    </div>
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

<style scoped>
/* Override Element Plus input styles for a darker, more premium look */
:deep(.custom-input .el-input__wrapper) {
  background-color: rgba(255, 255, 255, 0.03) !important;
  border: 1px solid rgba(255, 255, 255, 0.1) !important;
  box-shadow: none !important;
  border-radius: 0.75rem !important;
  transition: all 0.2s ease;
}

:deep(.custom-input .el-input__wrapper.is-focus) {
  background-color: rgba(6, 182, 212, 0.05) !important;
  border-color: rgba(6, 182, 212, 0.5) !important;
  box-shadow: 0 0 0 1px rgba(6, 182, 212, 0.5) !important;
}

:deep(.custom-input .el-input__inner) {
  background: transparent !important;
  box-shadow: none !important;
  color: #ffffff !important;
  caret-color: #22d3ee;
}

:deep(.custom-input .el-input__inner::placeholder) {
  color: rgba(255, 255, 255, 0.3) !important;
}

:deep(.custom-input .el-input__inner:-webkit-autofill),
:deep(.custom-input .el-input__inner:-webkit-autofill:hover),
:deep(.custom-input .el-input__inner:-webkit-autofill:focus) {
  border: 0 !important;
  -webkit-text-fill-color: #ffffff !important;
  caret-color: #22d3ee !important;
  /* box-shadow: 0 0 0 1000px rgb(3 18 18) inset !important; */
  transition: background-color 9999s ease-in-out 0s;
}

:deep(.custom-input .el-input__password) {
  color: rgba(203, 213, 225, 0.8);
}

.mask-gradient-left {
  mask-image: linear-gradient(to right, transparent, black 40%);
  -webkit-mask-image: linear-gradient(to right, transparent, black 40%);
}
</style>
