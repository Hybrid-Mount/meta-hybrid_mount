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

import { Show, For } from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import { ICONS } from "../lib/constants";
import "./TopBar.css";
import "@material/web/icon/icon.js";
import "@material/web/iconbutton/icon-button.js";
import "@material/web/dialog/dialog.js";
import "@material/web/list/list.js";
import "@material/web/list/list-item.js";
import "@material/web/button/text-button.js";

interface MdDialogElement extends HTMLElement {
  show: () => void;
  close: () => void;
}

export default function TopBar() {
  let langDialogRef: MdDialogElement | undefined;

  function openLangDialog() {
    langDialogRef?.show();
  }

  function closeLangDialog() {
    langDialogRef?.close();
  }

  function setLang(code: string) {
    uiStore.setLang(code);
    closeLangDialog();
  }

  return (
    <>
      <header class="top-bar">
        <div class="top-bar-content">
          <h1 class="screen-title">
            {uiStore.L?.common?.appName ?? "Hybrid Mount"}
          </h1>
          <div class="top-actions">
            <md-icon-button
              onClick={openLangDialog}
              title={uiStore.L?.common?.language ?? "Language"}
            >
              <md-icon>
                <svg viewBox="0 0 24 24">
                  <path d={ICONS.translate} />
                </svg>
              </md-icon>
            </md-icon-button>
          </div>
        </div>
      </header>

      <div class="dialog-container">
        <md-dialog ref={langDialogRef} class="lang-dialog">
          <div slot="headline">{uiStore.L?.common?.language || "Language"}</div>

          <div slot="content" class="lang-list-container">
            <md-list>
              <For each={uiStore.availableLanguages}>
                {(l) => (
                  <md-list-item
                    class="lang-option"
                    type="button"
                    onClick={() => setLang(l.code)}
                  >
                    <div slot="headline">{l.name}</div>
                    <Show when={uiStore.lang === l.code}>
                      <md-icon slot="end">
                        <svg viewBox="0 0 24 24">
                          <path d={ICONS.check} />
                        </svg>
                      </md-icon>
                    </Show>
                  </md-list-item>
                )}
              </For>
            </md-list>
          </div>

          <div slot="actions">
            <md-text-button onClick={closeLangDialog}>
              {uiStore.L?.common?.cancel || "Cancel"}
            </md-text-button>
          </div>
        </md-dialog>
      </div>
    </>
  );
}
