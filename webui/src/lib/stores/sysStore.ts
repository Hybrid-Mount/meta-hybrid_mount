import { createSignal, createRoot } from "solid-js";
import { API } from "../api";
import type { InitPayload } from "../api/contracts";
import { APP_VERSION } from "../constants_gen";
import { uiStore } from "./uiStore";
import {
  isBoolean,
  isRecord,
  isString,
  isStringArray,
} from "../api/core/guards";
import { buildModeStats, buildMountedCount } from "../api/codec/runtimeCodec";
import type { OverlayMode, StorageStatus, SystemInfo } from "../types";

const createSysStore = () => {
  const [version, setVersion] = createSignal(APP_VERSION);
  const [storage, setStorage] = createSignal<StorageStatus>({ type: null });
  const [systemInfo, setSystemInfo] = createSignal<SystemInfo>({
    kernel: "-",
    selinux: "-",
    mountBase: "-",
    activeMounts: [],
  });
  const [activePartitions, setActivePartitions] = createSignal<string[]>([]);
  const [loading, setLoading] = createSignal(false);
  let pendingLoad: Promise<void> | null = null;
  let pendingVersionLoad: Promise<void> | null = null;
  let hasLoaded = false;
  let hasLoadedVersion = false;

  function loadFromInit(payload: InitPayload) {
    if (isString(payload.version)) {
      setVersion(payload.version);
      hasLoadedVersion = true;
    } else {
      console.warn("sysStore: init payload missing version");
    }
    const status = isRecord(payload.status) ? payload.status : null;
    if (status) {
      const modeStats = buildModeStats(status);
      setStorage({
        type:
          isString(status.storage_mode) && status.storage_mode.trim()
            ? (status.storage_mode as StorageStatus["type"])
            : "unknown",
        supported_modes: ["tmpfs", "ext4"],
        modeStats,
        mountedCount: buildMountedCount(status, modeStats),
      });
      setActivePartitions(
        isStringArray(status.active_mounts) ? status.active_mounts : [],
      );
    } else {
      console.warn("sysStore: init payload missing status object");
    }

    const sysInfo = isRecord(payload.system_info) ? payload.system_info : null;
    if (sysInfo) {
      setSystemInfo({
        kernel: isString(sysInfo.kernel) ? sysInfo.kernel : "Unknown",
        selinux: isString(sysInfo.selinux) ? sysInfo.selinux : "Unknown",
        mountBase: isString(sysInfo.mount_base) ? sysInfo.mount_base : "-",
        activeMounts: isStringArray(sysInfo.active_mounts)
          ? sysInfo.active_mounts
          : [],
        tmpfs_xattr_supported: isBoolean(sysInfo.tmpfs_xattr_supported)
          ? sysInfo.tmpfs_xattr_supported
          : undefined,
        supported_overlay_modes:
          Array.isArray(sysInfo.supported_overlay_modes) &&
          sysInfo.supported_overlay_modes.every(isString)
            ? (sysInfo.supported_overlay_modes as OverlayMode[])
            : ["tmpfs", "ext4"],
      });
      hasLoaded = true;
    } else {
      console.warn("sysStore: init payload missing system_info");
    }
  }

  async function loadStatus() {
    if (pendingLoad) return pendingLoad;

    setLoading(true);
    pendingLoad = (async () => {
      try {
        const [storageResult, systemInfoResult] = await Promise.allSettled([
          API.getStorageUsage(),
          API.getSystemInfo(),
        ]);
        let loadedAny = false;
        let failedAny = false;

        if (storageResult.status === "fulfilled") {
          setStorage(storageResult.value);
          loadedAny = true;
        } else {
          failedAny = true;
          console.error("Failed to load storage status", storageResult.reason);
        }

        if (systemInfoResult.status === "fulfilled") {
          setSystemInfo(systemInfoResult.value);
          setActivePartitions(systemInfoResult.value.activeMounts || []);
          loadedAny = true;
        } else {
          failedAny = true;
          console.error("Failed to load system info", systemInfoResult.reason);
        }

        hasLoaded = hasLoaded || loadedAny;

        if (failedAny) {
          uiStore.showToast(
            uiStore.L.status?.loadError || "Failed to load system status",
            "error",
          );
        }
      } catch (e) {
        console.error("Failed to load system status", e);
        uiStore.showToast(
          uiStore.L.status?.loadError || "Failed to load system status",
          "error",
        );
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
    const status = isRecord(state) ? state : null;
    if (!status) return;
    const modeStats = buildModeStats(status);
    setStorage({
      type:
        isString(status.storage_mode) && (status.storage_mode as string).trim()
          ? (status.storage_mode as StorageStatus["type"])
          : "unknown",
      supported_modes: ["tmpfs", "ext4"],
      modeStats,
      mountedCount: buildMountedCount(status, modeStats),
    });
    setActivePartitions(
      isStringArray(status.active_mounts) ? status.active_mounts : [],
    );
  }

  return {
    get version() {
      return version();
    },
    get storage() {
      return storage();
    },
    get systemInfo() {
      return systemInfo();
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
