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

import { createSignal, createMemo, createRoot } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { API } from "../api";
import { getErrorMessage } from "../api/core/error";
import { uiStore } from "./uiStore";
import type { Module, ModuleRules, ModeStats } from "../types";

const createModuleStore = () => {
  const [modules, setModulesStore] = createStore<Module[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  let pendingLoad: Promise<boolean> | null = null;
  let hasLoaded = false;
  let activeSaves = 0;
  let saveRevision = 0;
  const latestSaveRevisions = new Map<string, number>();
  const moduleSaveTails = new Map<string, Promise<void>>();

  function beginSave() {
    activeSaves += 1;
    setSaving(true);
  }

  function endSave() {
    activeSaves -= 1;
    setSaving(activeSaves > 0);
  }

  const modeStats = createMemo((): ModeStats => {
    const stats: ModeStats = {
      overlay: 0,
      magic: 0,
      kasumi: 0,
      blacklisted: 0,
    };
    for (const m of modules) {
      if (m.is_mounted && m.mode in stats) {
        stats[m.mode as keyof ModeStats]++;
      }
    }
    return stats;
  });

  async function loadModules(force = false) {
    if (pendingLoad) return pendingLoad;
    if (hasLoaded && !force) return true;

    setLoading(true);
    pendingLoad = (async () => {
      try {
        const data = await API.scanModules();
        setModulesStore(reconcile(data));
        hasLoaded = true;
        return true;
      } catch (e: unknown) {
        uiStore.showToast(
          getErrorMessage(e, uiStore.L.modules.scanError),
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

  function ensureModulesLoaded() {
    if (hasLoaded) return Promise.resolve(true);
    return loadModules();
  }

  function invalidate() {
    hasLoaded = false;
  }

  function saveCurrentModuleRules(moduleId: string, rules: ModuleRules) {
    const revision = ++saveRevision;
    latestSaveRevisions.set(moduleId, revision);
    beginSave();

    // Preserve click order for each module. Otherwise two quick mode changes
    // can reach the daemon out of order and leave the older choice persisted.
    const previous = moduleSaveTails.get(moduleId) ?? Promise.resolve();
    const operation = previous.then(async () => {
      try {
        await API.saveModuleRules(moduleId, rules);
        if (latestSaveRevisions.get(moduleId) === revision) {
          uiStore.showToast(uiStore.L.common.saved, "success");
        }
        return true;
      } catch (e: unknown) {
        const isLatest = latestSaveRevisions.get(moduleId) === revision;
        if (isLatest) {
          uiStore.showToast(
            getErrorMessage(e, uiStore.L.modules.saveFailed),
            "error",
          );
        }
        // A newer queued choice owns the final UI state and any rollback.
        return !isLatest;
      } finally {
        endSave();
      }
    });
    const tail = operation.then(() => undefined);
    moduleSaveTails.set(moduleId, tail);
    void tail.finally(() => {
      if (moduleSaveTails.get(moduleId) === tail) {
        moduleSaveTails.delete(moduleId);
      }
    });
    return operation;
  }

  return {
    get modules() {
      return modules;
    },
    set modules(v) {
      setModulesStore(reconcile(v));
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
    get modeStats() {
      return modeStats();
    },
    ensureModulesLoaded,
    invalidate,
    loadModules,
    saveModuleRules: saveCurrentModuleRules,
  };
};

export const moduleStore = createRoot(createModuleStore);
