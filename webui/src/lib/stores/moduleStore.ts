/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createMemo, createRoot } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { API } from "../api";
import { uiStore } from "./uiStore";
import type { Module, ModeStats } from "../types";

const createModuleStore = () => {
  const [modules, setModulesStore] = createStore<Module[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);

  const modeStats = createMemo((): ModeStats => {
    const stats = { auto: 0, magic: 0 };
    modules.forEach((m) => {
      if (!m.is_mounted) return;
      if (m.mode === "auto") stats.auto++;
      else if (m.mode === "magic") stats.magic++;
    });
    return stats;
  });

  async function loadModules() {
    setLoading(true);
    try {
      const data = await API.scanModules();
      setModulesStore(reconcile(data));
    } catch (e: any) {
      uiStore.showToast(
        uiStore.L.modules?.scanError || "Failed to load modules",
        "error",
      );
    }
    setLoading(false);
  }

  async function saveModules() {
    setSaving(true);
    try {
      await API.saveModules(modules);
      uiStore.showToast(uiStore.L.common?.saved || "Saved", "success");
    } catch (e: any) {
      uiStore.showToast(
        uiStore.L.modules?.saveFailed || "Failed to save module modes",
        "error",
      );
    }
    setSaving(false);
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
    get modeStats() {
      return modeStats();
    },
    loadModules,
    saveModules,
  };
};

export const moduleStore = createRoot(createModuleStore);
