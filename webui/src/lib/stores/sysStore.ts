/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createRoot } from "solid-js";
import { API } from "../api";
import { APP_VERSION } from "../constants_gen";
import type { StorageStatus, SystemInfo, DeviceInfo } from "../types";

const createSysStore = () => {
  const [device, setDevice] = createSignal<DeviceInfo>({
    model: "-",
    android: "-",
    kernel: "-",
    selinux: "-",
  });
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

  async function loadStatus() {
    setLoading(true);
    try {
      const d = await API.getDeviceStatus();
      setDevice(d);
      const v = await API.getVersion();
      setVersion(v);
      const s = await API.getStorageUsage();
      setStorage(s);
      const info = await API.getSystemInfo();
      setSystemInfo(info);
      setActivePartitions(info.activeMounts || []);
    } catch {}
    setLoading(false);
  }

  return {
    get device() {
      return device();
    },
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
    loadStatus,
  };
};

export const sysStore = createRoot(createSysStore);
