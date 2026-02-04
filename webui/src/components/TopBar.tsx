import { Show, For } from "solid-js";
import { store } from "../lib/store";
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
    store.setLang(code);
    closeLangDialog();
  }

  return (
    <>
      <header class="top-bar">
        <div class="top-bar-content">
          <h1 class="screen-title">{store.L?.common?.appName}</h1>
          <div class="top-actions">
            <md-icon-button
              onClick={openLangDialog}
              title={store.L?.common?.language}
              role="button"
              tabIndex={0}
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

      <md-dialog ref={langDialogRef} class="lang-dialog">
        <div slot="headline">{store.L?.common?.language || "Language"}</div>

        <div slot="content" class="lang-list-container">
          <md-list>
            <For each={store.availableLanguages}>
              {(l) => (
                <md-list-item
                  type="button"
                  onClick={() => setLang(l.code)}
                  style={{ cursor: "pointer" }}
                >
                  <div slot="headline">{l.name}</div>
                  <Show when={store.lang === l.code}>
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
            {store.L?.common?.cancel || "Cancel"}
          </md-text-button>
        </div>
      </md-dialog>
    </>
  );
}
