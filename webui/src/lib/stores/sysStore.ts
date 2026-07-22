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

import { createSignal, createRoot } from "solid-js";
import { API } from "../api";
import type { InitPayload } from "../api/contracts";
import { APP_VERSION } from "../constants_gen";
import { uiStore } from "./uiStore";
import { buildModeStats, buildMountedCount } from "../api/codec/runtimeCodec";
import { runtimeStateSchema } from "../api/schemas";
import type { StorageStatus, SystemInfo } from "../types";

const createSysStore = () => {
  const [version, setVersion] = createSignal(APP_VERSION);
  const [storage, setStorage] = createSignal<StorageStatus | null>(null);
  const [systemInfo, setSystemInfo] = createSignal<SystemInfo | null>(null);
  const [activePartitions, setActivePartitions] = createSignal<string[]>([]);
  const [loading, setLoading] = createSignal(false);
  let pendingLoad: Promise<void> | null = null;
  let pendingVersionLoad: Promise<void> | null = null;
  let hasLoaded = false;
  let hasLoadedVersion = false;

  function loadFromInit(payload: InitPayload) {
    setVersion(payload.version);
    hasLoadedVersion = true;
    const status = payload.status;
    setStorage({
      type: status.storage_mode,
      modeStats: buildModeStats(status),
      mountedCount: buildMountedCount(status),
    });
    setActivePartitions(status.active_mounts);

    const sysInfo = payload.system_info;
    setSystemInfo({
      kernel: sysInfo.kernel,
      selinux: sysInfo.selinux,
      mountBase: sysInfo.mount_base,
      activeMounts: sysInfo.active_mounts,
      tmpfs_xattr_supported: sysInfo.tmpfs_xattr_supported,
      supported_overlay_modes: sysInfo.supported_overlay_modes,
    });
    hasLoaded = true;
  }

  async function loadStatus() {
    if (pendingLoad) return pendingLoad;

    setLoading(true);
    pendingLoad = (async () => {
      try {
        const [nextStorage, nextSystemInfo] = await Promise.all([
          API.getStorageUsage(),
          API.getSystemInfo(),
        ]);
        setStorage(nextStorage);
        setSystemInfo(nextSystemInfo);
        setActivePartitions(nextSystemInfo.activeMounts);
        hasLoaded = true;
      } catch (e) {
        console.error("Failed to load system status", e);
        uiStore.showToast(uiStore.L.status.loadError, "error");
      } finally {
        setLoading(false);
        pendingLoad = null;
      }
    })();

    return pendingLoad;
  }

  async function loadVersion() {
    if (pendingVersionLoad) return pendingVersionLoad;

    pendingVersionLoad = (async () => {
      try {
        setVersion(await API.getVersion());
        hasLoadedVersion = true;
      } catch (e) {
        console.error("Failed to load version", e);
      } finally {
        pendingVersionLoad = null;
      }
    })();

    return pendingVersionLoad;
  }

  function ensureStatusLoaded() {
    if (hasLoaded) return Promise.resolve();
    return loadStatus();
  }

  function ensureVersionLoaded() {
    if (hasLoadedVersion) return Promise.resolve();
    return loadVersion();
  }

  function handleSseUpdate(state: unknown) {
    const status = runtimeStateSchema.parse(state);
    setStorage({
      type: status.storage_mode,
      modeStats: buildModeStats(status),
      mountedCount: buildMountedCount(status),
    });
    setActivePartitions(status.active_mounts);
  }

  return {
    get version() {
      return version();
    },
    get storage() {
      const value = storage();
      if (!value) throw new Error("Storage status has not been initialized");
      return value;
    },
    get systemInfo() {
      const value = systemInfo();
      if (!value) throw new Error("System info has not been initialized");
      return value;
    },
    get activePartitions() {
      return activePartitions();
    },
    get loading() {
      return loading();
    },
    ensureStatusLoaded,
    ensureVersionLoaded,
    loadFromInit,
    loadStatus,
    loadVersion,
    handleSseUpdate,
  };
};

export const sysStore = createRoot(createSysStore);
