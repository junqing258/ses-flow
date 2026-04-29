import "./styles/index.css";
import "./styles/element/index.scss";

import ElementPlus from "element-plus";
import { createApp } from "vue";

import App from "./App.vue";
import { i18n } from "./i18n";
import { createRouter, createWebHashHistory } from "vue-router";
import generatedRoutes from "~pages";
import { useAuth } from "@/composables/useAuth";

const router = createRouter({
  history: createWebHashHistory(import.meta.env.BASE_URL),
  routes: [
    ...generatedRoutes,
    {
      path: "/",
      redirect: { name: "workflow-list" },
    },
    {
      path: "/:pathMatch(.*)*",
      redirect: { name: "workflow-list" },
    },
  ],
});

router.beforeEach(async (to) => {
  const { initialize, isAuthenticated, isInitialized } = useAuth();
  if (!isInitialized.value) {
    await initialize();
  }
  if (to.meta.public) {
    if (to.name === "login" && isAuthenticated.value) {
      return { name: "workflow-list" };
    }
    return true;
  }
  if (!isAuthenticated.value) {
    return {
      name: "login",
      query: {
        redirect: to.fullPath,
      },
    };
  }
  return true;
});

const app = createApp(App);

app.use(i18n);
app.use(router);
app.use(ElementPlus);
app.mount("#app");
