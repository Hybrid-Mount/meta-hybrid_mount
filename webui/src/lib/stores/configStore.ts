/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createRoot } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { API } from "../api";
import { DEFAULT_CONFIG } from "../constants";
import { uiStore } from "./uiStore";
import type { AppConfig } from "../types";

const createConfigStore = () => {
  const [config, setConfigStore] = createStore<AppConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);

  async function loadConfig() {
    setLoading(true);
    try {
      const data = await API.loadConfig();
      setConfigStore(reconcile(data));
    } catch (e: any) {
      uiStore.showToast(
        uiStore.L.config?.loadError || "Failed to load config",
        "error",
      );
    }
    setLoading(false);
  }

  async function saveConfig() {
    setSaving(true);
    try {
      await API.saveConfig(config);
      uiStore.showToast(uiStore.L.common?.saved || "Saved", "success");
    } catch (e: any) {
      uiStore.showToast(
        uiStore.L.config?.saveFailed || "Failed to save config",
        "error",
      );
    }
    setSaving(false);
  }

  async function resetConfig() {
    setSaving(true);
    try {
      await API.resetConfig();
      await loadConfig();
      uiStore.showToast(
        uiStore.L.config?.resetSuccess || "Config reset to defaults",
        "success",
      );
    } catch (e: any) {
      uiStore.showToast(
        uiStore.L.config?.saveFailed || "Failed to reset config",
        "error",
      );
    }
    setSaving(false);
  }

  return {
    get config() {
      return config;
    },
    set config(v) {
      setConfigStore(reconcile(v));
    },
    get loading() {
      return loading();
    },
    get saving() {
      return saving();
    },
    loadConfig,
    saveConfig,
    resetConfig,
  };
};

export const configStore = createRoot(createConfigStore);
