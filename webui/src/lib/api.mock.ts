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
import type { RuntimeStatePayload } from "./api/schemas";
import type {
  AppConfig,
  ModeStats,
  Module,
  ModuleRules,
  StorageStatus,
  SystemInfo,
} from "./types";

const delay = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms));

let mockConfig: AppConfig = structuredClone(DEFAULT_CONFIG);

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

function mockModule(
  module: Partial<Module> & Pick<Module, "id" | "mode" | "name">,
): Module {
  return {
    version: "1.0.0",
    author: "Developer",
    description: "This is a mock module for testing.",
    is_mounted: module.mode !== "ignore",
    enabled: true,
    is_blacklisted: false,
    rules: {
      default_mode: module.mode,
      paths: {},
    },
    ...module,
  };
}

function buildMockModules(): Module[] {
  return [
    mockModule({
      id: "magisk_module_1",
      name: "Example Module",
      mode: "magic",
      rules: {
        default_mode: "magic",
        paths: { "system/fonts": "overlay" },
      },
    }),
    mockModule({
      id: "overlay_module_2",
      name: "System UI Overlay",
      version: "2.5",
      author: "Google",
      description: "Changes system colors.",
      mode: "overlay",
    }),
    mockModule({
      id: "disabled_module",
      name: "Unmounted Module",
      version: "0.1",
      author: "Tester",
      description: "This module is not mounted.",
      mode: "ignore",
    }),
    mockModule({
      id: "blacklisted_example",
      name: "Blacklisted Module",
      version: "0.5",
      author: "Unknown",
      description: "This module is blacklisted and skipped during mount.",
      mode: "ignore",
      enabled: false,
      is_blacklisted: true,
    }),
  ];
}

function buildModeStats(): ModeStats {
  return {
    overlay: 1,
    magic: 1,
    blacklisted: 1,
  };
}

function buildMockRuntimeState(): RuntimeStatePayload {
  return {
    timestamp: 1,
    pid: 1234,
    storage_mode: "ext4",
    mount_point: "/data/adb/hybrid-mount/mnt",
    overlay_modules: ["overlay_module_2"],
    magic_modules: ["magisk_module_1"],
    custom_mounts: [],
    skip_mount_modules: [],
    blacklisted_modules: ["blacklisted_example"],
    active_mounts: ["system", "product"],
    tmpfs_xattr_supported: false,
    mount_stats: {
      total_mounts: 2,
      successful_mounts: 2,
      failed_mounts: 0,
      tmpfs_created: 0,
      files_mounted: 0,
      dirs_mounted: 0,
      symlinks_created: 0,
      overlayfs_mounts: 1,
      ignored_entries: 0,
    },
    mode_stats: {
      overlayfs: 1,
      magicmount: 1,
      blacklisted: 1,
    },
    daemon: {
      alive: true,
      socket_path: "/data/adb/hybrid-mount/run/hybrid-mount.sock",
      last_refresh_ts: 1,
    },
  };
}

export const MockAPI: AppAPI = {
  wakeDaemon: () => delay(20),

  async init() {
    await delay(80);
    const systemInfo = buildMockSystemInfo();
    return {
      status: buildMockRuntimeState(),
      config: structuredClone(mockConfig),
      version: APP_VERSION,
      system_info: {
        kernel: systemInfo.kernel,
        selinux: systemInfo.selinux,
        mount_base: systemInfo.mountBase,
        active_mounts: systemInfo.activeMounts,
        tmpfs_xattr_supported: systemInfo.tmpfs_xattr_supported,
        supported_overlay_modes: systemInfo.supported_overlay_modes,
      },
    };
  },

  async loadConfig(): Promise<AppConfig> {
    await delay(60);
    return structuredClone(mockConfig);
  },

  async patchConfig(patch: Record<string, unknown>): Promise<AppConfig> {
    await delay(60);
    mockConfig = { ...mockConfig, ...(patch as Partial<AppConfig>) };
    return structuredClone(mockConfig);
  },

  async resetConfig(): Promise<void> {
    await delay(60);
    mockConfig = structuredClone(DEFAULT_CONFIG);
  },

  async scanModules(): Promise<Module[]> {
    await delay(80);
    return buildMockModules();
  },

  async saveModuleRules(moduleId: string, rules: ModuleRules): Promise<void> {
    await delay(60);
    console.log(`[Mock] Rules saved for ${moduleId}:`, rules);
  },

  async getVersion(): Promise<string> {
    await delay(20);
    return APP_VERSION;
  },

  async getStorageUsage(): Promise<StorageStatus> {
    await delay(60);
    return {
      type: "ext4",
      modeStats: buildModeStats(),
      mountedCount: 2,
    };
  },

  async getSystemInfo(): Promise<SystemInfo> {
    await delay(60);
    return buildMockSystemInfo();
  },

  async openLink(url: string): Promise<void> {
    await delay(20);
    window.open(url, "_blank", "noopener,noreferrer");
  },

  reboot: () => delay(20),
};
