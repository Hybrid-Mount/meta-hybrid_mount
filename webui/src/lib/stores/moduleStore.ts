// SPDX-License-Identifier: Apache-2.0

import { ref } from "vue";
import type { Module, ModuleRule } from "../types";
import { API } from "../api";
import { uiStore } from "./uiStore";
import { MODULE_ID } from "../constants";

const modules = ref<Module[]>([]);
const loading = ref(false);
let pendingLoad: Promise<void> | null = null;
let hasLoaded = false;

async function loadModules(): Promise<void> {
  if (pendingLoad) {
    return pendingLoad;
  }

  loading.value = true;
  pendingLoad = (async () => {
    try {
      const data = await API.scanModules();
      modules.value = [...data];
      hasLoaded = true;
    } catch {
      uiStore.showToast("Failed to scan modules");
    } finally {
      loading.value = false;
      pendingLoad = null;
    }
  })();

  return pendingLoad;
}

function ensureModulesLoaded(): Promise<void> {
  if (hasLoaded) return Promise.resolve();
  return loadModules();
}

async function saveModuleRules(moduleId: string, rules: ModuleRule): Promise<boolean> {
  try {
    await API.saveModuleRules(moduleId, rules);
    const module = modules.value.find((item) => item.id === moduleId);
    if (module) {
      module.rules = {
        default_mode: rules.default_mode ?? "overlay",
        paths: Object.fromEntries(
          Object.entries(rules.paths).map(([path, mode]) => [path, mode]),
        ),
      };
    }
    return true;
  } catch {
    return false;
  }
}

export const moduleStore = {
  get modules() {
    return modules.value.filter((module) => module.id !== MODULE_ID);
  },
  get loading() {
    return loading.value;
  },
  get hasLoaded() {
    return hasLoaded;
  },
  ensureModulesLoaded,
  loadModules,
  saveModuleRules,
};
