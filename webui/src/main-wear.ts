// SPDX-License-Identifier: Apache-2.0

import { createApp } from "vue";
import i18n from "./locales";
import "./style.css";
import WearApp from "./ui/wear/App.vue";
import { uiStore } from "./lib/stores/uiStore";
import { loadOptionalManagerColors } from "./lib/managerColors";

async function init(): Promise<void> {
  await uiStore.init();
  uiStore.setUiStyle("md3");
  loadOptionalManagerColors();
  createApp(WearApp).use(i18n).mount("#app");
}

void init();
