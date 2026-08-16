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

import { createSignal, createEffect, createMemo, For } from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import { configStore } from "../lib/stores/configStore";
import { sysStore } from "../lib/stores/sysStore";
import { moduleStore } from "../lib/stores/moduleStore";
import { ICONS } from "../lib/constants";
import "./ConfigTab.css";
import "@material/web/textfield/outlined-text-field.js";
import "@material/web/icon/icon.js";
import "@material/web/ripple/ripple.js";
import type { OverlayMode, AppConfig } from "../lib/types";

export default function ConfigTab() {
  const [lastSavedConfig, setLastSavedConfig] = createSignal("");
  let mountSourceInputRef: HTMLElement | undefined;

  const isValidPath = (p: string) => !p || (p.startsWith("/") && p.length > 1);
  const invalidModuleDir = createMemo(
    () => !isValidPath(configStore.config.moduledir),
  );
  createEffect(() => {
    if (!configStore.loading && configStore.config && !lastSavedConfig()) {
      setLastSavedConfig(JSON.stringify(configStore.config));
    }
  });

  function updateConfig<K extends keyof AppConfig>(
    key: K,
    value: AppConfig[K],
  ) {
    configStore.config = { ...configStore.config, [key]: value };
  }

  async function refreshModulesForConfigChange() {
    const shouldReload = moduleStore.hasLoaded;
    moduleStore.invalidate();
    if (shouldReload) {
      await moduleStore.loadModules(true);
    }
  }

  async function saveCurrentConfig(): Promise<boolean> {
    if (invalidModuleDir()) {
      uiStore.showToast(uiStore.L.config.invalidPath, "error");
      return false;
    }
    const prevSnapshot = lastSavedConfig();
    const saved = await configStore.saveConfig(configStore.config, {
      showSuccess: false,
    });
    if (saved) {
      setLastSavedConfig(JSON.stringify(configStore.config));
    } else if (prevSnapshot) {
      try {
        configStore.config = JSON.parse(prevSnapshot) as AppConfig;
      } catch (e) {
        console.warn("Failed to restore config snapshot", e);
      }
    }
    return saved;
  }

  async function handleTextFieldCommit(key: keyof AppConfig) {
    const saved = await saveCurrentConfig();
    if (saved && key === "moduledir") {
      await refreshModulesForConfigChange();
    }
  }

  async function toggle(key: keyof AppConfig) {
    const currentVal = configStore.config[key] as boolean;
    updateConfig(key, !currentVal);
    const saved = await saveCurrentConfig();
    if (!saved) {
      updateConfig(key, currentVal);
    }
  }

  async function toggleDaemonMode() {
    const current = configStore.config.daemon_startup_mode;
    const next = current === "persistent" ? "on-demand" : "persistent";
    updateConfig("daemon_startup_mode", next as "on-demand" | "persistent");
    const saved = await saveCurrentConfig();
    if (!saved) {
      updateConfig("daemon_startup_mode", current);
    }
  }

  async function setOverlayMode(mode: string) {
    const prev = configStore.config.overlay_mode;
    updateConfig("overlay_mode", mode as OverlayMode);
    const saved = await saveCurrentConfig();
    if (!saved) {
      updateConfig("overlay_mode", prev);
    }
  }

  const availableModes = createMemo(() => {
    const storageModes = (sysStore.storage as any)?.supported_modes;
    let modes: OverlayMode[];

    if (storageModes && Array.isArray(storageModes)) {
      modes = storageModes as OverlayMode[];
    } else {
      modes =
        sysStore.systemInfo?.supported_overlay_modes ??
        (["tmpfs", "ext4"] as OverlayMode[]);
    }

    if (sysStore.systemInfo?.tmpfs_xattr_supported === false) {
      modes = modes.filter((m) => m !== "tmpfs");
    }

    return modes;
  });

  const MODE_DESCS: Record<OverlayMode, string> = {
    tmpfs: "RAM-based. Fastest I/O, reset on reboot.",
    ext4: "Loopback image. Persistent, saves RAM.",
  };

  return (
    <>
      <div class="config-container">
        <section class="config-group">
          <div class="config-card">
            <div class="card-header">
              <div class="card-icon">
                <md-icon>
                  <svg viewBox="0 0 24 24">
                    <path d={ICONS.modules} />
                  </svg>
                </md-icon>
              </div>
              <div class="card-text">
                <span class="card-title">{uiStore.L.config.moduleDir}</span>
                <span class="card-desc">
                  {uiStore.L.config?.moduleDirDesc ??
                    "Set the directory where modules are stored"}
                </span>
              </div>
            </div>

            <div class="input-stack">
              <md-outlined-text-field
                label={uiStore.L.config.moduleDir}
                value={configStore.config.moduledir}
                onInput={(e: Event) =>
                  updateConfig(
                    "moduledir",
                    (e.currentTarget as HTMLInputElement).value,
                  )
                }
                onChange={() => handleTextFieldCommit("moduledir")}
                error={invalidModuleDir()}
                supporting-text={
                  invalidModuleDir()
                    ? uiStore.L.config?.invalidModuleDir || "Invalid Path"
                    : ""
                }
                class="full-width-field"
              >
                <md-icon slot="leading-icon">
                  <svg viewBox="0 0 24 24">
                    <path d={ICONS.modules} />
                  </svg>
                </md-icon>
              </md-outlined-text-field>
            </div>
          </div>

          <div class="config-card">
            <div class="card-header">
              <div class="card-icon">
                <md-icon>
                  <svg viewBox="0 0 24 24">
                    <path d={ICONS.ksu} />
                  </svg>
                </md-icon>
              </div>
              <div class="card-text">
                <span class="card-title">{uiStore.L.config.mountSource}</span>
                <span class="card-desc">
                  {uiStore.L.config?.mountSourceDesc ??
                    "Global mount source namespace (e.g. KSU)"}
                </span>
              </div>
            </div>

            <div class="input-stack">
              <md-outlined-text-field
                ref={(el) => (mountSourceInputRef = el)}
                label={uiStore.L.config.mountSource}
                value={configStore.config.mountsource}
                onInput={(e: Event) =>
                  updateConfig(
                    "mountsource",
                    (e.currentTarget as HTMLInputElement).value,
                  )
                }
                onChange={() => handleTextFieldCommit("mountsource")}
                onFocus={() => {
                  setTimeout(() => {
                    mountSourceInputRef?.scrollIntoView({
                      behavior: "smooth",
                      block: "center",
                    });
                  }, 300);
                }}
                class="full-width-field"
              >
                <md-icon slot="leading-icon">
                  <svg viewBox="0 0 24 24">
                    <path d={ICONS.ksu} />
                  </svg>
                </md-icon>
              </md-outlined-text-field>
            </div>
          </div>
        </section>

        <section class="config-group">
          <div class="config-card">
            <div class="card-header">
              <div class="card-icon">
                <md-icon>
                  <svg viewBox="0 0 24 24">
                    <path d={ICONS.save} />
                  </svg>
                </md-icon>
              </div>
              <div class="card-text">
                <span class="card-title">
                  {uiStore.L.config?.overlayMode || "Overlay Mode"}
                </span>
                <span class="card-desc">
                  {uiStore.L.config?.overlayModeDesc ||
                    "Select backing storage strategy"}
                </span>
              </div>
            </div>
            <div class="mode-selector">
              <For each={availableModes()}>
                {(mode) => (
                  <button
                    class={`mode-item ${configStore.config.overlay_mode === mode ? "selected" : ""}`}
                    onClick={() => setOverlayMode(mode)}
                    type="button"
                  >
                    <md-ripple></md-ripple>
                    <div class="mode-info">
                      <span class="mode-title">
                        {uiStore.L.config?.[`mode_${mode}`] || mode}
                      </span>
                      <span class="mode-desc">
                        {uiStore.L.config?.[`mode_${mode}Desc`] ||
                          MODE_DESCS[mode]}
                      </span>
                    </div>
                    <div class="mode-check">
                      <md-icon>
                        <svg viewBox="0 0 24 24">
                          <path d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z" />
                        </svg>
                      </md-icon>
                    </div>
                  </button>
                )}
              </For>
            </div>
          </div>
        </section>

        <section class="config-group">
          <div class="options-grid">
            <button
              class={`option-tile clickable tertiary ${configStore.config.disable_umount ? "active" : ""}`}
              onClick={() => toggle("disable_umount")}
              type="button"
            >
              <md-ripple></md-ripple>
              <div class="tile-top">
                <div class="tile-icon">
                  <md-icon>
                    <svg viewBox="0 0 24 24">
                      <path d={ICONS.anchor} />
                    </svg>
                  </md-icon>
                </div>
              </div>
              <div class="tile-bottom">
                <span class="tile-label">{uiStore.L.config.disableUmount}</span>
              </div>
            </button>

            <button
              class={`option-tile clickable tertiary ${configStore.config.daemon_startup_mode === "persistent" ? "active" : ""}`}
              onClick={toggleDaemonMode}
              type="button"
            >
              <md-ripple></md-ripple>
              <div class="tile-top">
                <div class="tile-icon">
                  <md-icon>
                    <svg viewBox="0 0 24 24">
                      <path d={ICONS.power} />
                    </svg>
                  </md-icon>
                </div>
              </div>
              <div class="tile-bottom">
                <span class="tile-label">
                  {uiStore.L.config?.daemonStartupMode || "Persistent Daemon"}
                </span>
              </div>
            </button>
          </div>
        </section>
      </div>
    </>
  );
}
