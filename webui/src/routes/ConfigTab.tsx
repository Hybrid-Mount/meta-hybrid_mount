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

import { createSignal, createEffect, createMemo, For, Show } from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import { configStore } from "../lib/stores/configStore";
import { sysStore } from "../lib/stores/sysStore";
import { moduleStore } from "../lib/stores/moduleStore";
import { ICONS } from "../lib/constants";
import { ENABLE_KASUMI } from "../lib/constants_gen";
import { features } from "../lib/features";
import { getCookie, setCookie } from "../lib/cookies";
import { getErrorMessage } from "../lib/api/core/error";
import { API } from "../lib/api";
import { kasumiStore } from "../lib/stores/kasumiStore";
import "./ConfigTab.css";
import "@material/web/textfield/outlined-text-field.js";
import "@material/web/icon/icon.js";
import "@material/web/ripple/ripple.js";
import "@material/web/dialog/dialog.js";
import "@material/web/button/text-button.js";
import "@material/web/button/filled-button.js";
import type { OverlayMode, AppConfig, CustomBindMount } from "../lib/types";

const KASUMI_WARNING_COOKIE = "mhm_kasumi_warning_ack";

export default function ConfigTab() {
  const [lastSavedConfig, setLastSavedConfig] =
    createSignal<Partial<AppConfig> | null>(null);
  const [showKasumiWarning, setShowKasumiWarning] = createSignal(false);
  const [kasumiPending, setKasumiPending] = createSignal(false);
  const [customSourceDraft, setCustomSourceDraft] = createSignal("");
  const [customTargetDraft, setCustomTargetDraft] = createSignal("");
  let mountSourceInputRef: HTMLElement | undefined;

  const isValidPath = (p: string) => !p || (p.startsWith("/") && p.length > 1);
  const isValidRequiredPath = (p: string) => p.startsWith("/") && p.length > 1;
  const invalidModuleDir = createMemo(
    () => !isValidPath(configStore.config.moduledir),
  );
  const tmpfsXattrUnsupported = createMemo(
    () => sysStore.systemInfo?.tmpfs_xattr_supported === false,
  );

  createEffect(() => {
    if (!configStore.loading && configStore.config && !lastSavedConfig()) {
      recordSavedConfig();
    }
  });

  function recordSavedConfig() {
    setLastSavedConfig({
      ...configStore.config,
      custom_mounts: cloneCustomMounts(configStore.config.custom_mounts),
    });
  }

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

  function shouldApplyRuntime(key: keyof AppConfig) {
    return key !== "daemon_startup_mode" && key !== "custom_mounts";
  }

  async function saveConfigField<K extends keyof AppConfig>(
    key: K,
    value: AppConfig[K],
    previousValue: AppConfig[K],
  ): Promise<boolean> {
    if (key === "moduledir" && invalidModuleDir()) {
      uiStore.showToast(uiStore.L.config.invalidPath, "error");
      return false;
    }

    const saved = await configStore.patchConfig(
      { [key]: value } as Partial<AppConfig>,
      {
        applyRuntime: shouldApplyRuntime(key),
        showSuccess: false,
      },
    );
    if (saved) {
      recordSavedConfig();
    } else {
      updateConfig(key, previousValue);
    }
    return saved;
  }

  async function handleTextFieldCommit<K extends keyof AppConfig>(key: K) {
    const value = configStore.config[key];
    const previous = (lastSavedConfig()?.[key] ?? value) as AppConfig[K];
    if (Object.is(value, previous)) return;

    const saved = await saveConfigField(key, value, previous);
    if (saved && key === "moduledir") {
      await refreshModulesForConfigChange();
    }
  }

  async function toggle<K extends keyof AppConfig>(key: K) {
    const currentVal = configStore.config[key] as boolean;
    const nextValue = !currentVal as AppConfig[K];
    updateConfig(key, nextValue);
    await saveConfigField(key, nextValue, currentVal as AppConfig[K]);
  }

  async function toggleDaemonMode() {
    const current = configStore.config.daemon_startup_mode;
    const next = current === "persistent" ? "on-demand" : "persistent";
    updateConfig("daemon_startup_mode", next);
    await saveConfigField("daemon_startup_mode", next, current);
  }

  async function setOverlayMode(mode: string) {
    const prev = configStore.config.overlay_mode;
    const next = mode as OverlayMode;
    if (prev === next) return;

    updateConfig("overlay_mode", next);
    await saveConfigField("overlay_mode", next, prev);
  }

  function cloneCustomMounts(
    mounts: CustomBindMount[] | undefined,
  ): CustomBindMount[] {
    return (mounts ?? []).map((mount) => ({
      source: mount.source,
      target: mount.target,
    }));
  }

  const customMounts = createMemo(() =>
    cloneCustomMounts(configStore.config.custom_mounts),
  );

  function hasInvalidCustomMount(mounts: CustomBindMount[]) {
    return mounts.some(
      (mount) =>
        !isValidRequiredPath(mount.source) ||
        !isValidRequiredPath(mount.target),
    );
  }

  async function saveCustomMounts(
    nextMounts: CustomBindMount[],
    previousMounts: CustomBindMount[],
  ) {
    if (hasInvalidCustomMount(nextMounts)) {
      uiStore.showToast(
        uiStore.L.config?.invalidCustomMount || "Invalid custom mount path",
        "error",
      );
      updateConfig("custom_mounts", previousMounts);
      return false;
    }

    return saveConfigField("custom_mounts", nextMounts, previousMounts);
  }

  function updateCustomMount(
    index: number,
    key: keyof CustomBindMount,
    value: string,
  ) {
    const next = customMounts();
    if (!next[index]) return;
    next[index] = { ...next[index], [key]: value };
    updateConfig("custom_mounts", next);
  }

  async function commitCustomMounts() {
    const previous = cloneCustomMounts(lastSavedConfig()?.custom_mounts);
    const next = customMounts();
    if (JSON.stringify(previous) === JSON.stringify(next)) return;
    await saveCustomMounts(next, previous);
  }

  async function addCustomMount() {
    const source = customSourceDraft().trim();
    const target = customTargetDraft().trim();
    const previous = customMounts();
    const next = [...previous, { source, target }];

    updateConfig("custom_mounts", next);
    const saved = await saveCustomMounts(next, previous);
    if (saved) {
      setCustomSourceDraft("");
      setCustomTargetDraft("");
    }
  }

  async function removeCustomMount(index: number) {
    const previous = customMounts();
    const next = previous.filter((_, i) => i !== index);
    updateConfig("custom_mounts", next);
    await saveCustomMounts(next, previous);
  }

  async function handleKasumiToggle() {
    const wantsEnable = !features.kasumiEnabled;

    if (wantsEnable && getCookie(KASUMI_WARNING_COOKIE) !== "1") {
      setShowKasumiWarning(true);
      return;
    }

    await applyKasumiToggle(wantsEnable);
  }

  async function applyKasumiToggle(enabled: boolean) {
    setShowKasumiWarning(false);
    setKasumiPending(true);
    try {
      await API.setKasumiEnabled(enabled);
      kasumiStore.setEnabledOptimistic(enabled);
      await kasumiStore.refreshStatus(false);
      features.setKasumiStatus(
        kasumiStore.enabled,
        Boolean(kasumiStore.status?.available),
        Boolean(kasumiStore.status?.kernel_supported),
      );
      if (enabled) {
        setCookie(KASUMI_WARNING_COOKIE, "1");
      }
      uiStore.showToast(
        uiStore.L.config?.kasumiConfigSaved || "Kasumi config saved.",
        "success",
      );
    } catch (e: unknown) {
      uiStore.showToast(
        getErrorMessage(e, uiStore.L.config?.saveFailed ?? "Failed to save"),
        "error",
      );
    } finally {
      setKasumiPending(false);
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
      <Show when={ENABLE_KASUMI && features.kasumiKernelSupported}>
        <div class="dialog-container">
          <md-dialog
            open={showKasumiWarning()}
            onclose={() => setShowKasumiWarning(false)}
            class="transparent-scrim"
          >
            <div slot="headline">
              {uiStore.L.config?.kasumiWarningTitle ??
                "Enable Experimental Kasumi?"}
            </div>
            <div slot="content">
              {uiStore.L.config?.kasumiWarningBody ??
                "Kasumi is experimental. Enabling it will expose the Kasumi tab, allow Kasumi-backed module routing, and permit LKM autoload. Continue only if you know what you are testing."}
            </div>
            <div slot="actions">
              <md-text-button onClick={() => setShowKasumiWarning(false)}>
                {uiStore.L.common?.cancel ?? "Cancel"}
              </md-text-button>
              <md-text-button onClick={() => applyKasumiToggle(true)}>
                {uiStore.L.config?.kasumiEnableConfirm ?? "Enable Kasumi"}
              </md-text-button>
            </div>
          </md-dialog>
        </div>
      </Show>

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

        <section class="config-group">
          <div class="config-card">
            <div class="card-header">
              <div class="card-icon">
                <md-icon>
                  <svg viewBox="0 0 24 24">
                    <path d={ICONS.mount_path} />
                  </svg>
                </md-icon>
              </div>
              <div class="card-text">
                <span class="card-title">
                  {uiStore.L.config?.customMounts || "Custom Bind Mounts"}
                </span>
                <span class="card-desc">
                  {uiStore.L.config?.customMountsDesc ||
                    "Bind explicit source paths onto existing targets"}
                </span>
              </div>
            </div>

            <div class="custom-mount-list">
              <Show
                when={customMounts().length > 0}
                fallback={
                  <div class="custom-mount-empty">
                    {uiStore.L.config?.customMountsEmpty ||
                      "No custom bind mounts configured."}
                  </div>
                }
              >
                <For each={customMounts()}>
                  {(mount, index) => (
                    <div class="custom-mount-row">
                      <div class="custom-mount-fields">
                        <md-outlined-text-field
                          label={
                            uiStore.L.config?.customMountSource || "Source"
                          }
                          value={mount.source}
                          onInput={(e: Event) =>
                            updateCustomMount(
                              index(),
                              "source",
                              (e.currentTarget as HTMLInputElement).value,
                            )
                          }
                          onChange={commitCustomMounts}
                          error={!isValidRequiredPath(mount.source)}
                          supporting-text={
                            !isValidRequiredPath(mount.source)
                              ? uiStore.L.config?.invalidPath || "Invalid path"
                              : ""
                          }
                          class="custom-mount-field"
                        />
                        <md-outlined-text-field
                          label={
                            uiStore.L.config?.customMountTarget || "Target"
                          }
                          value={mount.target}
                          onInput={(e: Event) =>
                            updateCustomMount(
                              index(),
                              "target",
                              (e.currentTarget as HTMLInputElement).value,
                            )
                          }
                          onChange={commitCustomMounts}
                          error={!isValidRequiredPath(mount.target)}
                          supporting-text={
                            !isValidRequiredPath(mount.target)
                              ? uiStore.L.config?.invalidPath || "Invalid path"
                              : ""
                          }
                          class="custom-mount-field"
                        />
                      </div>
                      <button
                        class="custom-mount-icon-button"
                        type="button"
                        title={uiStore.L.config?.removeCustomMount || "Remove"}
                        aria-label={
                          uiStore.L.config?.removeCustomMount || "Remove"
                        }
                        onClick={() => removeCustomMount(index())}
                      >
                        <md-icon>
                          <svg viewBox="0 0 24 24">
                            <path d={ICONS.delete} />
                          </svg>
                        </md-icon>
                      </button>
                    </div>
                  )}
                </For>
              </Show>

              <div class="custom-mount-add-row">
                <md-outlined-text-field
                  label={uiStore.L.config?.customMountSource || "Source"}
                  value={customSourceDraft()}
                  onInput={(e: Event) =>
                    setCustomSourceDraft(
                      (e.currentTarget as HTMLInputElement).value,
                    )
                  }
                  error={
                    customSourceDraft().length > 0 &&
                    !isValidRequiredPath(customSourceDraft())
                  }
                  supporting-text={
                    customSourceDraft().length > 0 &&
                    !isValidRequiredPath(customSourceDraft())
                      ? uiStore.L.config?.invalidPath || "Invalid path"
                      : ""
                  }
                  class="custom-mount-field"
                />
                <md-outlined-text-field
                  label={uiStore.L.config?.customMountTarget || "Target"}
                  value={customTargetDraft()}
                  onInput={(e: Event) =>
                    setCustomTargetDraft(
                      (e.currentTarget as HTMLInputElement).value,
                    )
                  }
                  onKeyDown={(e: KeyboardEvent) => {
                    if (e.key === "Enter") void addCustomMount();
                  }}
                  error={
                    customTargetDraft().length > 0 &&
                    !isValidRequiredPath(customTargetDraft())
                  }
                  supporting-text={
                    customTargetDraft().length > 0 &&
                    !isValidRequiredPath(customTargetDraft())
                      ? uiStore.L.config?.invalidPath || "Invalid path"
                      : ""
                  }
                  class="custom-mount-field"
                />
                <md-filled-button
                  onClick={addCustomMount}
                  disabled={
                    !isValidRequiredPath(customSourceDraft().trim()) ||
                    !isValidRequiredPath(customTargetDraft().trim())
                  }
                >
                  {uiStore.L.config?.addCustomMount || "Add"}
                </md-filled-button>
              </div>
            </div>
          </div>
        </section>

        <Show when={ENABLE_KASUMI && features.kasumiKernelSupported}>
          <section class="config-group">
            <div class="webui-label">
              {uiStore.L.config?.experimentalFeatures ||
                "Experimental Features"}
            </div>
            <div class="options-grid">
              <button
                class={`option-tile clickable secondary ${features.kasumiEnabled ? "active" : ""}`}
                onClick={handleKasumiToggle}
                disabled={kasumiPending()}
                type="button"
                aria-pressed={features.kasumiEnabled}
                aria-label={
                  uiStore.L.config?.kasumiMasterSwitch || "Enable Kasumi"
                }
              >
                <md-ripple></md-ripple>
                <div class="tile-top">
                  <div class="tile-icon">
                    <md-icon>
                      <svg viewBox="0 0 24 24">
                        <path
                          d={
                            features.kasumiEnabled
                              ? ICONS.snowflake_filled
                              : ICONS.snowflake
                          }
                        />
                      </svg>
                    </md-icon>
                  </div>
                </div>
                <div class="tile-bottom">
                  <span class="tile-label">
                    {uiStore.L.config?.kasumiMasterTitle ??
                      "Experimental Kasumi"}
                  </span>
                </div>
              </button>
            </div>
            <Show when={tmpfsXattrUnsupported()}>
              <div class="kasumi-restriction-note">
                <md-icon>
                  <svg viewBox="0 0 24 24">
                    <path d={ICONS.info} />
                  </svg>
                </md-icon>
                <span>
                  {uiStore.L.config?.kasumiTmpfsRestriction ??
                    "Per-module Kasumi mount is unavailable because tmpfs xattr is not supported on this kernel."}
                </span>
              </div>
            </Show>
          </section>
        </Show>
      </div>
    </>
  );
}
