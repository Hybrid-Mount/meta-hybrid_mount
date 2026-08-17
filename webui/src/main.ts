// SPDX-License-Identifier: Apache-2.0

import { createApp } from "vue";
import i18n, { initI18n } from "./locales";
import "./style.css";

import App from "./App.vue";
import { uiStore } from "./lib/stores/uiStore";

const app = createApp(App);
app.use(i18n);

const init = async () => {
  await uiStore.init();
  const savedLocale = localStorage.getItem("locale");
  await initI18n(savedLocale ?? undefined);
  app.mount("#app");
};

init();
