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

import { APP_VERSION } from "./constants_gen";
import { DEFAULT_CONFIG } from "./constants";
import type { AppAPI } from "./api/contracts";
import type {
  AppConfig,
  ModeStats,
  Module,
  ModuleRules,
  StorageStatus,
  SystemInfo,
} from "./types";

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function createMockState() {
  return {
    version: APP_VERSION,
    mountErrorsCleared: false,
  };
}

const mockState = createMockState();

function buildMockSystemInfo(): SystemInfo {
  return {
    kernel: "Linux localhost 6.12.0-android16-gki #1 SMP PREEMPT",
    selinux: "Enforcing",
    mountBase: "/data/adb/hybrid-mount/mnt",
    activeMounts: ["system", "product"],
    tmpfs_xattr_supported: false,
    supported_overlay_modes: ["ext4"],
  };
}

function buildMockModules(): Module[] {
  return [
    {
      id: "magisk_module_1",
      name: "Example Module",
      version: "1.0.0",
      author: "Developer",
      description: "This is a mock module for testing.",
      mode: "magic",
      is_mounted: true,
      enabled: true,
      source_path: "/data/adb/modules/magisk_module_1",
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
      mode: "overlay",
      is_mounted: true,
      enabled: true,
      source_path: "/data/adb/modules/overlay_module_2",
      rules: {
        default_mode: "overlay",
        paths: {},
      },
    },
    {
      id: "disabled_module",
      name: "Umount Module",
      version: "0.1",
      author: "Tester",
      description: "This module has a mount error.",
      mode: "ignore",
      is_mounted: false,
      enabled: true,
      source_path: "/data/adb/modules/disabled_module",
      mount_error: mockState.mountErrorsCleared
        ? undefined
        : "stage=execute; error=mock mount failure",
      suggest_ignore: mockState.mountErrorsCleared ? undefined : true,
      rules: {
        default_mode: "ignore",
        paths: {},
      },
    },
    {
      id: "blacklisted_example",
      name: "Blacklisted Module",
      version: "0.5",
      author: "Unknown",
      description: "This module is blacklisted and skipped during mount.",
      mode: "ignore",
      is_mounted: false,
      enabled: false,
      source_path: "/data/adb/modules/blacklisted_example",
      rules: {
        default_mode: "ignore",
        paths: {},
      },
    },
  ];
}

function buildModeStats(): ModeStats {
  return {
    overlay: 1,
    magic: 1,
    blacklisted: 1,
  };
}

export const MockAPI: AppAPI = {
  async wakeDaemon(): Promise<void> {
    await delay(20);
  },

  async init() {
    await delay(200);
    return {
      status: {
        storage_mode: "tmpfs",
        mount_point: "/data/adb/hybrid-mount/mnt",
        overlay_modules: ["overlay_module_2"],
        magic_modules: ["magisk_module_1"],
        mount_error_modules: mockState.mountErrorsCleared
          ? []
          : ["disabled_module"],
        blacklisted_modules: ["blacklisted_example"],
        active_mounts: ["system", "product"],
        tmpfs_xattr_supported: false,
        mode_stats: {
          overlayfs: 1,
          magicmount: 1,
          blacklisted: 1,
        },
      },
      config: { ...DEFAULT_CONFIG },
      version: mockState.version,
      system_info: buildMockSystemInfo(),
    };
  },

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

  async scanModules(_dir?: string): Promise<Module[]> {
    await delay(600);
    return buildMockModules();
  },

  async saveModules(modules: Module[]): Promise<void> {
    await delay(400);
    console.log("[Mock] Modules saved:", modules);
  },

  async saveModuleRules(moduleId: string, rules: ModuleRules): Promise<void> {
    await delay(400);
    console.log(`[Mock] Rules saved for ${moduleId}:`, rules);
  },

  async saveAllModuleRules(rules: Record<string, ModuleRules>): Promise<void> {
    await delay(400);
    console.log("[Mock] All module rules saved:", rules);
  },

  async getVersion(): Promise<string> {
    await delay(100);
    return mockState.version;
  },

  async getStorageUsage(): Promise<StorageStatus> {
    await delay(300);
    return {
      type: "ext4",
      supported_modes: ["tmpfs", "ext4"],
      modeStats: buildModeStats(),
      mountedCount: 3,
    };
  },

  async getSystemInfo(): Promise<SystemInfo> {
    await delay(300);
    return buildMockSystemInfo();
  },

  async clearMountErrors(): Promise<void> {
    await delay(180);
    mockState.mountErrorsCleared = true;
  },

  async openLink(url: string): Promise<void> {
    await delay(100);
    window.open(url, "_blank", "noopener,noreferrer");
  },

  async reboot(): Promise<void> {
    await delay(120);
  },
};
