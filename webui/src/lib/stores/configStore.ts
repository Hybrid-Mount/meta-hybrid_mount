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

const createConfigStore = () => {
  const [config, setConfigStore] = createStore<AppConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  let pendingLoad: Promise<boolean> | null = null;
  let hasLoaded = false;

  async function loadConfig(force = false) {
    if (pendingLoad) return pendingLoad;
    if (hasLoaded && !force) return true;

    setLoading(true);
    pendingLoad = (async () => {
      try {
        const data = await API.loadConfig();
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

  async function saveConfig(
    nextConfig: AppConfig = config,
    options: SaveConfigOptions = {},
  ) {
    const { showSuccess = true, showError = true } = options;
    const normalizedConfig = normalizeConfig(nextConfig);

    setSaving(true);
    try {
      await API.saveConfig(normalizedConfig);
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
      setSaving(false);
    }
  }

  async function resetConfig() {
    setSaving(true);
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
      setSaving(false);
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
    saveConfig,
    resetConfig,
  };
};

export const configStore = createRoot(createConfigStore);
