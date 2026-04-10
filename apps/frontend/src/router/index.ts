import { createRouter, createWebHashHistory } from "vue-router";

import AboutPage from "@/pages/AboutPage.vue";
import WorkflowEditorPage from "@/pages/WorkflowEditorPage.vue";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/workflow",
      name: "workflow",
      component: WorkflowEditorPage,
    },
    {
      path: "/about",
      name: "about",
      component: AboutPage,
    },
    {
      path: "/",
      redirect: "/workflow",
    },
    {
      path: "/:pathMatch(.*)*",
      redirect: "/workflow",
    },
  ],
});

export default router;
