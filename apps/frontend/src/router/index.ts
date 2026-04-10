import { createRouter, createWebHashHistory } from "vue-router";

import AboutPage from "@/pages/AboutPage.vue";
import WorkflowEditorPage from "@/pages/WorkflowEditorPage.vue";
import WorkflowListPage from "@/pages/WorkflowListPage.vue";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/workflow",
      name: "workflow-list",
      component: WorkflowListPage,
    },
    {
      path: "/workflow/new",
      name: "workflow-new",
      component: WorkflowEditorPage,
    },
    {
      path: "/workflow/:id",
      name: "workflow-editor",
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
