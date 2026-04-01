import { createRouter, createWebHashHistory } from "vue-router";

import AboutPage from "@/pages/AboutPage.vue";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/about",
      name: "about",
      component: AboutPage,
    },
    {
      path: "/",
      redirect: "/about",
    },
    {
      path: "/:pathMatch(.*)*",
      redirect: "/about",
    },
  ],
});

export default router;
