/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { createApp } from "vue";
// import Vconsole from "vconsole";
import i18n, { initI18n } from "./locales";
import "./style.css";
import "miuix-vue/style.css";
import App from "./App.vue";

const app = createApp(App);
app.use(i18n);
// new Vconsole(); // unless need to debug,dont uncomment it
const init = async () => {
  const savedLocale = localStorage.getItem("locale");
  await initI18n(savedLocale ?? undefined);

  app.mount("#app");
};

init();
