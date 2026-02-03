/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { APP_VERSION } from "./constants_gen";
import { DEFAULT_CONFIG } from "./constants";
import type {
  AppConfig,
  DeviceInfo,
  Module,
  StorageStatus,
  SystemInfo,
  ModuleRules,
} from "./types";

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export const MockAPI = {
  async loadConfig(): Promise<AppConfig> {
    await delay(300);
    return { ...DEFAULT_CONFIG };
  },
  async saveConfig(config: AppConfig): Promise<void> {
    await delay(500);
    console.log("[Mock] Config saved:", config);
  },
  async resetConfig(): Promise<void> {
    await delay(500);
    console.log("[Mock] Config reset to defaults");
  },
  async scanModules(_dir: string): Promise<Module[]> {
    await delay(600);
    return [
      {
        id: "magisk_module_1",
        name: "Example Module",
        version: "1.0.0",
        author: "Developer",
        description: "This is a mock module for testing.",
        mode: "magic",
        is_mounted: true,
        rules: {
          default_mode: "magic",
          paths: { "system/fonts": "overlay" },
        },
      },
      {
        id: "overlay_module_2",
        name: "System UI Overlay",
        version: "2.5",
        author: "Google",
        description: "Changes system colors.",
        mode: "auto",
        is_mounted: true,
        rules: {
          default_mode: "overlay",
          paths: {},
        },
      },
      {
        id: "disabled_module",
        name: "Unmounted Module",
        version: "0.1",
        author: "Tester",
        description: "This module is not mounted.",
        mode: "ignore",
        is_mounted: false,
        rules: {
          default_mode: "ignore",
          paths: {},
        },
      },
    ];
  },
  async saveModuleRules(moduleId: string, rules: ModuleRules): Promise<void> {
    await delay(400);
    console.log(`[Mock] Rules saved for ${moduleId}:`, rules);
  },
  async getDeviceStatus(): Promise<DeviceInfo> {
    await delay(300);
    return {
      model: "Pixel 8 Pro (Mock)",
      android: "14 (API 34)",
      kernel: "5.15.110-android14-11",
      selinux: "Enforcing",
    };
  },
  async getVersion(): Promise<string> {
    await delay(100);
    return APP_VERSION;
  },
  async getStorageUsage(): Promise<StorageStatus> {
    await delay(300);
    return {
      type: "erofs",
    };
  },
  async getSystemInfo(): Promise<SystemInfo> {
    await delay(300);
    return {
      kernel: "Linux localhost 5.15.0 #1 SMP PREEMPT",
      selinux: "Enforcing",
      mountBase: "/data/adb/meta-hybrid/mnt",
      activeMounts: ["system", "product"],
      zygisksuEnforce: "1",
      tmpfs_xattr_supported: false,
    };
  },
  async fetchSystemColor(): Promise<string | null> {
    await delay(100);
    return "#8AB4F8";
  },
};
