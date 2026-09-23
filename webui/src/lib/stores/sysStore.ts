// SPDX-License-Identifier: Apache-2.0

import { ref } from "vue";
import type {
  DefaultMountMode,
  DeviceInfo,
  InstallState,
  MountMode,
  RunState,
  SystemInfo,
} from "../types";
import { API } from "../api";
import { uiStore } from "./uiStore";
import { moduleStore } from "./moduleStore";

const device = ref<DeviceInfo>({ model: "-", android: "-", sdk: "-" });
const version = ref("...");
const systemInfo = ref<SystemInfo>({ kernel: "-", selinux: "-" });
const state = ref<RunState | null>(null);
const installState = ref<InstallState | null>(null);
/**
 * Why the last status load failed, or null after a successful one.
 *
 * The toast disappears on its own; the status banner reads this so a failed load stays visible
 * instead of leaving stale numbers on screen with no explanation.
 */
const loadError = ref<string | null>(null);
const loading = ref(false);
let pendingLoad: Promise<void> | null = null;
let hasLoaded = false;

async function loadStatus(): Promise<void> {
  if (pendingLoad) return pendingLoad;

  loading.value = true;
  pendingLoad = (async () => {
    try {
      const [baseDevice, nextVersion, info, nextState, nextInstall] = await Promise.all([
        API.getDeviceStatus(),
        API.getVersion(),
        API.getSystemInfo(),
        API.getStatus(),
        API.getInstallState(),
      ]);

      device.value = baseDevice;
      version.value = nextVersion;
      systemInfo.value = info;
      state.value = nextState;
      installState.value = nextInstall;
      loadError.value = null;
      hasLoaded = true;
    } catch (error) {
      loadError.value = error instanceof Error ? error.message : String(error);
      uiStore.showToast("Failed to load system status");
    } finally {
      loading.value = false;
      pendingLoad = null;
    }
  })();

  return pendingLoad;
}

function ensureStatusLoaded(): Promise<void> {
  if (hasLoaded) return Promise.resolve();
  return loadStatus();
}

async function rebootDevice(): Promise<void> {
  try {
    await API.reboot();
  } catch (error) {
    uiStore.showToast(error instanceof Error ? error.message : "Reboot failed");
  }
}

async function clearMountErrors(): Promise<number> {
  try {
    const removed = await API.clearMountErrors();
    if (state.value) {
      state.value.mount_error_modules = [];
      state.value.mount_error_reasons = {};
    }
    moduleStore.clearMountErrorFlags();
    return removed;
  } catch {
    uiStore.showToast("Failed to clear mount errors");
    return 0;
  }
}

export const sysStore = {
  get device() {
    return device.value;
  },
  get version() {
    return version.value;
  },
  get systemInfo() {
    return systemInfo.value;
  },
  get state() {
    return state.value;
  },
  get installState() {
    return installState.value;
  },
  get loadError() {
    return loadError.value;
  },
  get vfsSupported() {
    return installState.value?.vfs_supported === true;
  },
  get mountModes(): MountMode[] {
    return this.vfsSupported
      ? ["overlay", "magic", "vfs", "ignore"]
      : ["overlay", "magic", "ignore"];
  },
  get defaultMountModes(): DefaultMountMode[] {
    return this.vfsSupported ? ["overlay", "magic", "vfs"] : ["overlay", "magic"];
  },
  get loading() {
    return loading.value;
  },
  get hasLoaded() {
    return hasLoaded;
  },
  ensureStatusLoaded,
  loadStatus,
  rebootDevice,
  clearMountErrors,
};
