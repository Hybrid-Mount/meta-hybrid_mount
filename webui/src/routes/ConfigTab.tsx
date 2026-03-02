/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createEffect, createMemo, For } from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import { configStore } from "../lib/stores/configStore";
import { sysStore } from "../lib/stores/sysStore";
import { ICONS } from "../lib/constants";
import { API } from "../lib/api";
import ChipInput from "../components/ChipInput";
import BottomActions from "../components/BottomActions";
import "./ConfigTab.css";
import "@material/web/textfield/outlined-text-field.js";
import "@material/web/button/filled-button.js";
import "@material/web/iconbutton/filled-tonal-icon-button.js";
import "@material/web/iconbutton/icon-button.js";
import "@material/web/icon/icon.js";
import "@material/web/ripple/ripple.js";
import "@material/web/dialog/dialog.js";
import "@material/web/button/text-button.js";
import "@material/web/switch/switch.js";
import type { OverlayMode, AppConfig } from "../lib/types";

export default function ConfigTab() {
  const [initialConfigStr, setInitialConfigStr] = createSignal("");
  const [showResetConfirm, setShowResetConfirm] = createSignal(false);

  const isValidPath = (p: string) => !p || (p.startsWith("/") && p.length > 1);
  const invalidModuleDir = createMemo(
    () => !isValidPath(configStore.config.moduledir),
  );

  const isDirty = createMemo(() => {
    if (!initialConfigStr()) return false;
    return JSON.stringify(configStore.config) !== initialConfigStr();
  });

  createEffect(() => {
    if (!configStore.loading && configStore.config) {
      if (
        !initialConfigStr() ||
        initialConfigStr() === JSON.stringify(configStore.config)
      ) {
        setInitialConfigStr(JSON.stringify(configStore.config));
      }
    }
  });

  function updateConfig<K extends keyof AppConfig>(
    key: K,
    value: AppConfig[K],
  ) {
    configStore.config = { ...configStore.config, [key]: value };
  }

  function save() {
    if (invalidModuleDir()) {
      uiStore.showToast(uiStore.L.config.invalidPath, "error");
      return;
    }
    configStore.saveConfig().then(() => {
      setInitialConfigStr(JSON.stringify(configStore.config));
    });
  }

  function reload() {
    configStore.loadConfig().then(() => {
      setInitialConfigStr(JSON.stringify(configStore.config));
    });
  }

  function reset() {
    setShowResetConfirm(false);
    configStore.resetConfig().then(() => {
      setInitialConfigStr(JSON.stringify(configStore.config));
    });
  }

  function toggle(key: keyof AppConfig) {
    const currentVal = configStore.config[key] as boolean;
    const newVal = !currentVal;

    updateConfig(key, newVal);

    API.saveConfig({ ...configStore.config, [key]: newVal }).catch(() => {
      updateConfig(key, currentVal);
      uiStore.showToast(
        uiStore.L.config?.saveFailed || "Failed to update setting",
        "error",
      );
    });
  }

  function setOverlayMode(mode: string) {
    updateConfig("overlay_mode", mode as OverlayMode);
  }

  const availableModes = createMemo(() => {
    const storageModes = (sysStore.storage as any)?.supported_modes;
    let modes: OverlayMode[];

    if (storageModes && Array.isArray(storageModes)) {
      modes = storageModes as OverlayMode[];
    } else {
      modes =
        sysStore.systemInfo?.supported_overlay_modes ??
        (["tmpfs", "ext4", "erofs"] as OverlayMode[]);
    }

    if (sysStore.systemInfo?.tmpfs_xattr_supported === false) {
      modes = modes.filter((m) => m !== "tmpfs");
    }

    return modes;
  });

  const MODE_DESCS: Record<OverlayMode, string> = {
    tmpfs: "RAM-based. Fastest I/O, reset on reboot.",
    ext4: "Loopback image. Persistent, saves RAM.",
    erofs: "Read-only compressed. High performance, space saving.",
  };

  return (
    <>
      <md-dialog
        open={showResetConfirm()}
        onclose={() => setShowResetConfirm(false)}
        class="transparent-scrim"
      >
        <div slot="headline">
          {uiStore.L.config?.resetConfigTitle ?? "Reset Configuration?"}
        </div>
        <div slot="content">
          {uiStore.L.config?.resetConfigConfirm ??
            "This will reset all backend settings to defaults. Continue?"}
        </div>
        <div slot="actions">
          <md-text-button onClick={() => setShowResetConfirm(false)}>
            {uiStore.L.common?.cancel ?? "Cancel"}
          </md-text-button>
          <md-text-button onClick={reset}>
            {uiStore.L.config?.resetConfig ?? "Reset Config"}
          </md-text-button>
        </div>
      </md-dialog>

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
                label={uiStore.L.config.mountSource}
                value={configStore.config.mountsource}
                onInput={(e: Event) =>
                  updateConfig(
                    "mountsource",
                    (e.currentTarget as HTMLInputElement).value,
                  )
                }
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
                    <path d={ICONS.storage} />
                  </svg>
                </md-icon>
              </div>
              <div class="card-text">
                <span class="card-title">{uiStore.L.config.partitions}</span>
                <span class="card-desc">
                  {uiStore.L.config?.partitionsDesc ??
                    "Add partitions to mount"}
                </span>
              </div>
            </div>
            <div class="p-input">
              <ChipInput
                values={configStore.config.partitions}
                placeholder="e.g. product, system_ext..."
                onValuesChange={(vals) => updateConfig("partitions", vals)}
              />
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

          <div class="options-grid">
            <button
              class={`option-tile clickable tertiary ${configStore.config.disable_umount ? "active" : ""}`}
              onClick={() => toggle("disable_umount")}
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
          </div>
        </section>

        <section class="config-group">
          <div class="webui-label">{uiStore.L.config?.webui || "WebUI"}</div>
          <div class="options-grid">
            <button
              class={`option-tile clickable secondary ${uiStore.fixBottomNav ? "active" : ""}`}
              onClick={uiStore.toggleBottomNavFix}
            >
              <md-ripple></md-ripple>
              <div class="tile-top">
                <div class="tile-icon">
                  <md-icon>
                    <svg viewBox="0 0 24 24">
                      <path d="M21 5v14H3V5h18zm0-2H3c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM8 17h5v-6H8v6zm0-8h5V7H8v2zM6 17h2V7H6v10zm12-6h-2v6h2v-6zm0-4h-2v2h2V7z" />
                    </svg>
                  </md-icon>
                </div>
              </div>
              <div class="tile-bottom">
                <span class="tile-label">
                  {uiStore.L.config?.fixBottomNav || "Fix Bottom Nav"}
                </span>
              </div>
            </button>

            <button
              class="option-tile clickable error"
              onClick={() => setShowResetConfirm(true)}
              disabled={configStore.saving}
            >
              <md-ripple></md-ripple>
              <div class="tile-top">
                <div class="tile-icon">
                  <md-icon>
                    <svg viewBox="0 0 24 24">
                      <path d={ICONS.replay} />
                    </svg>
                  </md-icon>
                </div>
              </div>
              <div class="tile-bottom">
                <span class="tile-label">
                  {uiStore.L.config?.resetConfig || "Reset Config"}
                </span>
              </div>
            </button>
          </div>
        </section>
      </div>

      <BottomActions>
        <md-filled-tonal-icon-button
          onClick={reload}
          disabled={configStore.loading}
          title={uiStore.L.config.reload}
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
          disabled={configStore.saving || !isDirty()}
          role="button"
          tabIndex={0}
        >
          <md-icon slot="icon">
            <svg viewBox="0 0 24 24">
              <path d={ICONS.save} />
            </svg>
          </md-icon>
          {configStore.saving ? uiStore.L.common.saving : uiStore.L.config.save}
        </md-filled-button>
      </BottomActions>
    </>
  );
}
