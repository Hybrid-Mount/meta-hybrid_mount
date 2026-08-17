// SPDX-License-Identifier: Apache-2.0

import { ref } from "vue";
import type { AppConfig } from "../types";
import { API } from "../api";
import { DEFAULT_CONFIG } from "../constants";
import { uiStore } from "./uiStore";

const config = ref<AppConfig>({ ...DEFAULT_CONFIG });
const loading = ref(false);
const saving = ref(false);

function setConfig(next: AppConfig): void {
  config.value = { ...next };
}

async function loadConfig(): Promise<void> {
  loading.value = true;
  try {
    const data = await API.loadConfig();
    config.value = { ...data };
  } catch {
    uiStore.showToast("Failed to load config");
  }
  loading.value = false;
}

async function saveConfig(): Promise<boolean> {
  saving.value = true;
  try {
    await API.saveConfig(config.value);
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
    config.value = { ...DEFAULT_CONFIG };
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
  loadConfig,
  saveConfig,
  resetConfig,
};
