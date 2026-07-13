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

import { createSignal, createRoot } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { API } from "../api";
import type { InitPayload } from "../api/contracts";
import { normalizeConfig } from "../api/codec/configCodec";
import { DEFAULT_CONFIG } from "../constants";
import { getErrorMessage } from "../api/core/error";
import { uiStore } from "./uiStore";
import type { AppConfig } from "../types";

interface SaveConfigOptions {
  showSuccess?: boolean;
  showError?: boolean;
}

interface PatchConfigOptions extends SaveConfigOptions {
  applyRuntime?: boolean;
}

const createConfigStore = () => {
  const [config, setConfigStore] = createStore<AppConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  let pendingLoad: Promise<boolean> | null = null;
  let hasLoaded = false;
  let activeSaves = 0;
  let patchRevision = 0;
  const fieldRevisions = new Map<keyof AppConfig, number>();

  function beginSave() {
    activeSaves += 1;
    setSaving(true);
  }

  function endSave() {
    activeSaves -= 1;
    setSaving(activeSaves > 0);
  }

  function invalidatePatchResponses() {
    patchRevision += 1;
    fieldRevisions.clear();
  }

  async function loadConfig(force = false) {
    if (pendingLoad) return pendingLoad;
    if (hasLoaded && !force) return true;

    setLoading(true);
    pendingLoad = (async () => {
      try {
        const data = await API.loadConfig();
        invalidatePatchResponses();
        setConfigStore(reconcile(normalizeConfig(data)));
        hasLoaded = true;
        return true;
      } catch (e: unknown) {
        uiStore.showToast(
          getErrorMessage(
            e,
            uiStore.L.config?.loadError ?? "Failed to load config",
          ),
          "error",
        );
        return false;
      } finally {
        setLoading(false);
        pendingLoad = null;
      }
    })();

    return pendingLoad;
  }

  function loadFromInit(payload: InitPayload) {
    if (payload.config != null) {
      const normalized = normalizeConfig(payload.config);
      invalidatePatchResponses();
      setConfigStore(reconcile(normalized));
      hasLoaded = true;
    } else {
      console.warn("configStore: init payload missing config");
    }
  }

  function ensureConfigLoaded() {
    if (hasLoaded) return Promise.resolve(true);
    return loadConfig();
  }

  function invalidate() {
    hasLoaded = false;
  }

  function setField<K extends keyof AppConfig>(key: K, value: AppConfig[K]) {
    setConfigStore(key, value);
  }

  async function patchConfig(
    patch: Partial<AppConfig>,
    options: PatchConfigOptions = {},
  ) {
    const {
      showSuccess = true,
      showError = true,
      applyRuntime = true,
    } = options;
    const revision = ++patchRevision;
    const fields = Object.keys(patch) as (keyof AppConfig)[];
    fields.forEach((field) => fieldRevisions.set(field, revision));

    beginSave();
    try {
      const updated = await API.patchConfig(patch as Record<string, unknown>, {
        applyRuntime,
      });
      const normalized = normalizeConfig(updated);
      fields.forEach((field) => {
        if (fieldRevisions.get(field) === revision) {
          setField(field, normalized[field]);
        }
      });
      if (showSuccess) {
        uiStore.showToast(uiStore.L.common?.saved || "Saved", "success");
      }
      return true;
    } catch (e: unknown) {
      if (showError) {
        uiStore.showToast(
          getErrorMessage(
            e,
            uiStore.L.config?.saveFailed ?? "Failed to save config",
          ),
          "error",
        );
      }
      return false;
    } finally {
      endSave();
    }
  }

  async function resetConfig() {
    invalidatePatchResponses();
    beginSave();
    try {
      await API.resetConfig();
      invalidate();
      const loaded = await loadConfig(true);
      if (!loaded) {
        return false;
      }
      uiStore.showToast(
        uiStore.L.config?.resetSuccess || "Config reset to defaults",
        "success",
      );
      return true;
    } catch (e: unknown) {
      uiStore.showToast(
        getErrorMessage(
          e,
          uiStore.L.config?.saveFailed ?? "Failed to reset config",
        ),
        "error",
      );
      return false;
    } finally {
      endSave();
    }
  }

  return {
    get config() {
      return config;
    },
    set config(v) {
      setConfigStore(reconcile(normalizeConfig(v)));
    },
    get loading() {
      return loading();
    },
    get saving() {
      return saving();
    },
    get hasLoaded() {
      return hasLoaded;
    },
    ensureConfigLoaded,
    invalidate,
    loadConfig,
    loadFromInit,
    setField,
    patchConfig,
    resetConfig,
  };
};

export const configStore = createRoot(createConfigStore);
