// SPDX-License-Identifier: Apache-2.0
import { ref, shallowRef } from "vue";
import { API } from "../api";
import type { RuntimeAction, RuntimeStatus } from "../types";
import { moduleStore } from "./moduleStore";
import { sysStore } from "./sysStore";

const status = shallowRef<RuntimeStatus | null>(null);
const loading = ref(false);
const busy = ref(false);
const refreshing = ref(false);
const loadError = ref<string | null>(null);
const actionError = shallowRef<{ moduleId: string; message: string } | null>(null);
let pendingLoad: Promise<void> | null = null;
let pendingRefresh: Promise<void> | null = null;

async function loadRuntimeStatus(): Promise<void> {
  if (pendingLoad) return pendingLoad;
  loading.value = true;
  pendingLoad = (async () => {
    try {
      status.value = await API.getRuntimeStatus();
      loadError.value = null;
    } catch (error) {
      status.value = null;
      loadError.value = error instanceof Error ? error.message : String(error);
    } finally {
      loading.value = false;
      pendingLoad = null;
    }
  })();
  return pendingLoad;
}

async function refresh(): Promise<void> {
  if (pendingRefresh) return pendingRefresh;
  refreshing.value = true;
  pendingRefresh = (async () => {
    // Drain reads begun before the mutation, then request fresh snapshots.
    const results = await Promise.allSettled([
      (async () => {
        if (moduleStore.loading) await moduleStore.loadModules();
        await moduleStore.loadModules();
        if (moduleStore.loadError) throw new Error(moduleStore.loadError);
      })(),
      (async () => {
        if (sysStore.loading) await sysStore.loadStatus();
        await sysStore.loadStatus();
        if (sysStore.loadError) throw new Error(sysStore.loadError);
      })(),
      loadRuntimeStatus(),
    ]);
    const failed = results.find((result) => result.status === "rejected");
    if (failed?.status === "rejected") {
      status.value = null;
      loadError.value =
        failed.reason instanceof Error ? failed.reason.message : String(failed.reason);
    }
    refreshing.value = false;
    pendingRefresh = null;
  })();
  return pendingRefresh;
}

async function runAction(moduleId: string, action: RuntimeAction): Promise<boolean> {
  const module = status.value?.modules.find((item) => item.id === moduleId);
  if (
    busy.value ||
    loading.value ||
    refreshing.value ||
    !status.value?.supported ||
    !module?.eligible
  )
    return false;
  if ((action === "load") === module.active) return false;
  busy.value = true;
  actionError.value = null;
  let succeeded = false;
  let generation: number | null = null;
  try {
    generation = (await API.runtimeAction(moduleId, action)).generation;
    succeeded = true;
  } catch (error) {
    actionError.value = {
      moduleId,
      message: error instanceof Error ? error.message : String(error),
    };
  } finally {
    // Even a rejected transaction may have changed the provider or invalidated its ledger.
    try {
      await refresh();
      if (generation !== null && status.value && status.value.generation < generation) {
        status.value = null;
        loadError.value =
          "Runtime snapshot is stale; refresh before making another change";
      }
    } finally {
      busy.value = false;
    }
  }
  return succeeded && !loadError.value;
}

export const runtimeStore = {
  get status() {
    return status.value;
  },
  get loading() {
    return loading.value || refreshing.value;
  },
  get busy() {
    return busy.value;
  },
  get loadError() {
    return loadError.value;
  },
  get actionError() {
    return actionError.value;
  },
  loadRuntimeStatus,
  refresh,
  runAction,
};
