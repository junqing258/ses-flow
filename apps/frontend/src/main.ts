import "./styles/index.css";
import "./styles/element/index.scss";

import ElementPlus from "element-plus";
import { createApp } from "vue";

import App from "./App.vue";
import { i18n } from "./i18n";
import { createRouter, createWebHashHistory } from "vue-router";
import generatedRoutes from "~pages";

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

const app = createApp(App);

app.use(i18n);
app.use(router);
app.use(ElementPlus);
app.mount("#app");
