import {
  createSignal,
  onMount,
  createEffect,
  createMemo,
  Show,
} from "solid-js";
import { store } from "../lib/store";
import { ICONS } from "../lib/constants";
import Skeleton from "../components/Skeleton";
import BottomActions from "../components/BottomActions";
import "./HymoFSTab.css";

import "@material/web/list/list.js";
import "@material/web/list/list-item.js";
import "@material/web/switch/switch.js";
import "@material/web/button/filled-tonal-button.js";
import "@material/web/button/text-button.js";
import "@material/web/button/filled-button.js";
import "@material/web/iconbutton/filled-tonal-icon-button.js";
import "@material/web/icon/icon.js";
import "@material/web/dialog/dialog.js";

export default function HymoFSTab() {
  const [showRmmodConfirm, setShowRmmodConfirm] = createSignal(false);
  const [initialConfigStr, setInitialConfigStr] = createSignal("");

  const isDirty = createMemo(() => {
    if (!initialConfigStr()) return false;
    return JSON.stringify(store.config) !== initialConfigStr();
  });

  createEffect(() => {
    if (!store.loading.config && store.config) {
      if (
        !initialConfigStr() ||
        initialConfigStr() === JSON.stringify(store.config)
      ) {
        setInitialConfigStr(JSON.stringify(store.config));
      }
    }
  });

  onMount(() => {
    reload();
  });

  function reload() {
    store.loadStatus();
    store.loadConfig().then(() => {
      setInitialConfigStr(JSON.stringify(store.config));
    });
  }

  function save() {
    store.saveConfig().then(() => {
      setInitialConfigStr(JSON.stringify(store.config));
    });
  }

  function toggleDebug() {
    store.config = {
      ...store.config,
      hymofs: {
        ...store.config.hymofs,
        debug: !store.config.hymofs.debug,
      },
    };
  }

  function toggleStealth() {
    store.config = {
      ...store.config,
      hymofs: {
        ...store.config.hymofs,
        stealth: !store.config.hymofs.stealth,
      },
    };
  }

  return (
    <>
      <md-dialog
        open={showRmmodConfirm()}
        onclose={() => setShowRmmodConfirm(false)}
        class="transparent-scrim"
      >
        <div slot="headline">
          {store.L?.hymofs?.rmmodTitle ?? "Unload HymoFS?"}
        </div>
        <div slot="content">
          {store.L?.hymofs?.rmmodConfirm ??
            "Are you sure you want to unload the HymoFS kernel module?"}
        </div>
        <div slot="actions">
          <md-text-button onClick={() => setShowRmmodConfirm(false)}>
            {store.L?.common?.cancel ?? "Cancel"}
          </md-text-button>
          <md-text-button
            onClick={() => {
              setShowRmmodConfirm(false);
              store.rmmodHymofs();
            }}
          >
            {store.L?.common?.confirm ?? "Confirm"}
          </md-text-button>
        </div>
      </md-dialog>

      <div class="hymofs-container">
        <div class="status-card">
          <div class="card-header">
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d="M22,11h-4.17l3.24-3.24-1.41-1.42L15,11h-2V9l4.66-4.66-1.42-1.41L13,6.17V2h-2v4.17L7.76,2.93,6.34,4.34,11,9v2H9L4.34,6.34,2.93,7.76,6.17,11H2v2h4.17l-3.24,3.24,1.41,1.42L9,13h2v2l-4.66,4.66,1.42,1.41L11,17.83V22h2v-4.17l3.24,3.24,1.42-1.41L13,15v-2h2l4.66,4.66,1.41-1.42L17.83,13H22V11z" />
              </svg>
            </md-icon>
            <span class="card-title">
              HymoFS {store.L?.status?.sysInfoTitle ?? "Status"}
            </span>
          </div>
          <div class="info-row">
            <span class="info-key">
              {store.L?.hymofs?.loaded ?? "Module Loaded"}
            </span>
            <Show
              when={!store.loading.status}
              fallback={<Skeleton width="60px" height="16px" />}
            >
              <span
                class={`info-val ${store.systemInfo?.hymofs_state?.loaded ? "active" : "inactive"}`}
              >
                {store.systemInfo?.hymofs_state?.loaded
                  ? (store.L?.common?.yes ?? "Yes")
                  : (store.L?.common?.no ?? "No")}
              </span>
            </Show>
          </div>
          <div class="info-row">
            <span class="info-key">
              {store.L?.hymofs?.version ?? "Version"}
            </span>
            <Show
              when={!store.loading.status}
              fallback={<Skeleton width="60px" height="16px" />}
            >
              <span class="info-val">
                {store.systemInfo?.hymofs_state?.version || "-"}
              </span>
            </Show>
          </div>
        </div>

        <div class="config-card">
          <div class="card-header">
            <span class="card-title">
              {store.L?.config?.title ?? "Configuration"}
            </span>
          </div>
          <md-list>
            <md-list-item>
              <div slot="headline">
                {store.L?.hymofs?.debugMode ?? "Debug Mode"}
              </div>
              <div slot="supporting-text">
                {store.L?.hymofs?.debugModeDesc ??
                  "Enable verbose logging for HymoFS."}
              </div>
              <md-switch
                slot="end"
                selected={store.config?.hymofs?.debug || false}
                onChange={toggleDebug}
              ></md-switch>
            </md-list-item>
            <md-list-item>
              <div slot="headline">
                {store.L?.hymofs?.stealthMode ?? "Stealth Mode"}
              </div>
              <div slot="supporting-text">
                {store.L?.hymofs?.stealthModeDesc ??
                  "Hide HymoFS mount points from standard detection."}
              </div>
              <md-switch
                slot="end"
                selected={store.config?.hymofs?.stealth || false}
                onChange={toggleStealth}
              ></md-switch>
            </md-list-item>
          </md-list>
        </div>

        <div class="action-card">
          <md-filled-tonal-button
            class="danger-btn"
            onClick={() => setShowRmmodConfirm(true)}
          >
            <md-icon slot="icon">
              <svg viewBox="0 0 24 24">
                <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z" />
              </svg>
            </md-icon>
            {store.L?.hymofs?.rmmodBtn ?? "Unload HymoFS Module"}
          </md-filled-tonal-button>
        </div>
      </div>

      <BottomActions>
        <md-filled-tonal-icon-button
          onClick={reload}
          disabled={store.loading.config || store.loading.status}
          title={store.L?.logs?.refresh ?? "Refresh"}
          role="button"
          tabIndex={0}
        >
          <md-icon>
            <svg viewBox="0 0 24 24">
              <path d={ICONS.refresh} />
            </svg>
          </md-icon>
        </md-filled-tonal-icon-button>

        <div class="spacer"></div>

        <md-filled-button
          onClick={save}
          disabled={store.saving.config || !isDirty()}
          role="button"
          tabIndex={0}
        >
          <md-icon slot="icon">
            <svg viewBox="0 0 24 24">
              <path d={ICONS.save} />
            </svg>
          </md-icon>
          {store.saving.config
            ? store.L?.common?.saving
            : (store.L?.config?.save ?? "Save")}
        </md-filled-button>
      </BottomActions>
    </>
  );
}
