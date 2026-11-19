// SPDX-License-Identifier: Apache-2.0

import { createApp } from "vue";
import { exec } from "kernelsu";
import i18n from "./locales";
import "./style.css";

import App from "./App.vue";
import { uiStore } from "./lib/stores/uiStore";
import { useWearLayout } from "./lib/wearLayout";

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
  const [wear] = await Promise.all([
    useWearLayout(location.search, document.documentElement.clientWidth, async () => {
      const result = await exec("pm has-feature android.hardware.type.watch");
      return result.errno === 0 && result.stdout.trim() === "true";
    }),
    uiStore.init(),
  ]);
  loadOptionalManagerColors();
  createApp(App, { wear }).use(i18n).mount("#app");
};

init();
