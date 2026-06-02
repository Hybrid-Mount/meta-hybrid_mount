import { createSignal, createMemo, createRoot } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { API } from "../api";
import { normalizeMountMode } from "../api/core/guards";
import { getErrorMessage } from "../api/core/error";
import { uiStore } from "./uiStore";
import type { Module, ModeStats } from "../types";

const createModuleStore = () => {
  const [modules, setModulesStore] = createStore<Module[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  let pendingLoad: Promise<boolean> | null = null;
  let hasLoaded = false;

  function normalizeModule(module: Module): Module {
    return {
      ...module,
      mode: normalizeMountMode(module.mode),
      rules: {
        ...module.rules,
        default_mode: normalizeMountMode(module.rules.default_mode),
      },
    };
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
        const data = (await API.scanModules()).map((module) =>
          normalizeModule(module as Module),
        );
        setModulesStore(reconcile(data));
        hasLoaded = true;
        return true;
      } catch (e: unknown) {
        uiStore.showToast(
          getErrorMessage(
            e,
            uiStore.L.modules?.scanError ?? "Failed to load modules",
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

  function ensureModulesLoaded() {
    if (hasLoaded) return Promise.resolve(true);
    return loadModules();
  }

  function invalidate() {
    hasLoaded = false;
  }

  async function saveCurrentModules() {
    setSaving(true);
    try {
      await API.saveModules(modules);
      uiStore.showToast(uiStore.L.common?.saved || "Saved", "success");
      return true;
    } catch (e: unknown) {
      uiStore.showToast(
        getErrorMessage(
          e,
          uiStore.L.modules?.saveFailed ?? "Failed to save module modes",
        ),
        "error",
      );
      return false;
    } finally {
      setSaving(false);
    }
  }

  return {
    get modules() {
      return modules;
    },
    set modules(v) {
      setModulesStore(reconcile(v.map(normalizeModule)));
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
    saveModules: saveCurrentModules,
  };
};

export const moduleStore = createRoot(createModuleStore);
