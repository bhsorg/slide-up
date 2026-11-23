import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";

import "./styles/tokens.css";
import "./styles/base.css";

if (import.meta.env.DEV) {
  document.body.classList.add("dev-mode");
}

const app = createApp(App);
app.use(createPinia());
app.mount("#app");
