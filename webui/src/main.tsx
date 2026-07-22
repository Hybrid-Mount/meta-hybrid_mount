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

import { render } from "solid-js/web";
import "./init";
import App from "./App.tsx";
import { uiStore } from "./lib/stores/uiStore";
import "./app.css";
import "./layout.css";

await uiStore.init();

const root = document.getElementById("app");

if (!(root instanceof HTMLElement)) {
  throw new Error("Missing #app root element");
}

render(() => <App />, root);
