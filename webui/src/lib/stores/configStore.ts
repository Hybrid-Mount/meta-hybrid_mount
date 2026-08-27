// SPDX-License-Identifier: Apache-2.0

import { ref } from "vue";
import type { AppConfig } from "../types";
import { API } from "../api";
import { cloneAppConfig } from "../config";
import { DEFAULT_CONFIG } from "../constants";
import { uiStore } from "./uiStore";

const config = ref<AppConfig>(cloneAppConfig(DEFAULT_CONFIG));
const loading = ref(false);
const saving = ref(false);
let pendingLoad: Promise<void> | null = null;
let hasLoaded = false;

function setConfig(next: AppConfig): void {
  config.value = cloneAppConfig(next);
}

async function loadConfig(): Promise<void> {
  if (pendingLoad) return pendingLoad;
  loading.value = true;
  pendingLoad = (async () => {
    try {
      const data = await API.loadConfig();
      config.value = cloneAppConfig(data);
      hasLoaded = true;
    } catch (error) {
      console.error("configStore: failed to load config", error);
      uiStore.showToast("Failed to load config");
    } finally {
      loading.value = false;
      pendingLoad = null;
    }
  })();
  return pendingLoad;
}

function ensureConfigLoaded(): Promise<void> {
  return hasLoaded ? Promise.resolve() : loadConfig();
}

async function saveConfig(): Promise<boolean> {
  saving.value = true;
  try {
    await API.saveConfig(config.value);
    hasLoaded = true;
    return true;
  } catch {
    return false;
  } finally {
    saving.value = false;
  }
}

async function resetConfig(): Promise<boolean> {
  saving.value = true;
  try {
    await API.genConfig();
    config.value = {
      ...cloneAppConfig(DEFAULT_CONFIG),
      tmpfs_xattr_supported: config.value.tmpfs_xattr_supported,
    };
    hasLoaded = true;
    return true;
  } catch {
    return false;
  } finally {
    saving.value = false;
  }
}

export const configStore = {
  get config() {
    return config.value;
  },
  setConfig,
  get loading() {
    return loading.value;
  },
  get saving() {
    return saving.value;
  },
  get hasLoaded() {
    return hasLoaded;
  },
  loadConfig,
  ensureConfigLoaded,
  saveConfig,
  resetConfig,
};
