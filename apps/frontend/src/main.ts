import "./styles.css";
import "vue-sonner/style.css";

import { createApp } from "vue";

import App from "./App.vue";
import { installApm } from "./lib/apm";
import router from "./router";

const app = createApp(App);

installApm(app, router);
app.use(router);
app.mount("#app");
