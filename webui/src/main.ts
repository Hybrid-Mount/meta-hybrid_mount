// SPDX-License-Identifier: Apache-2.0

import { createApp } from "vue";
import i18n, { initI18n } from "./locales";
import "./style.css";

import App from "./App.vue";
import { uiStore } from "./lib/stores/uiStore";

const app = createApp(App);
app.use(i18n);

function loadOptionalManagerColors(): void {
  if (document.head.querySelector('link[data-manager-colors="true"]')) return;

  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = "https://mui.kernelsu.org/internal/colors.css";
  link.dataset.managerColors = "true";
  link.addEventListener("error", () => link.remove(), { once: true });
  document.head.append(link);
}

const init = async () => {
  await uiStore.init();
  const savedLocale = localStorage.getItem("locale");
  await initI18n(savedLocale ?? undefined);
  loadOptionalManagerColors();
  app.mount("#app");
};

init();
