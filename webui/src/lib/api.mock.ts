// SPDX-License-Identifier: Apache-2.0

import type { AppAPI, AppConfig, Module, ModuleRule } from "./types";
import { DEFAULT_CONFIG } from "./constants";

const MOCK_DELAY = 300;
const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * `VITE_MOCK_STATUS_ERROR=1 pnpm dev` renders the status error banner.
 *
 * The mock is the only place a simulated failure may live: production builds never load this
 * module, so a shipped WebUI can only show problems the device really reported.
 */
const MOCK_STATUS_ERROR = import.meta.env.VITE_MOCK_STATUS_ERROR === "1";

export const MockAPI: AppAPI = {
  loadConfig: async () => {
    await delay(MOCK_DELAY);
    console.log("[MockAPI] loadConfig");
    return {
      ...DEFAULT_CONFIG,
      tmpfs_xattr_supported: true,
      rules: {
        "youtube-revanced": {
          default_mode: "magic",
          paths: { "system/etc/hosts": "overlay" },
        },
      },
    };
  },

  saveConfig: async (config: AppConfig) => {
    await delay(MOCK_DELAY);
    console.log("[MockAPI] saveConfig:", config);
  },

  genConfig: async () => {
    await delay(MOCK_DELAY);
    console.log("[MockAPI] genConfig");
  },

  saveModuleRules: async (moduleId: string, rules: ModuleRule) => {
    await delay(MOCK_DELAY);
    console.log("[MockAPI] saveModuleRules:", moduleId, rules);
  },

  scanModules: async (): Promise<Module[]> => {
    await delay(MOCK_DELAY);
    console.log("[MockAPI] scanModules");
    return [
      {
        id: "youtube-revanced",
        name: "YouTube ReVanced",
        version: "v18.20.39",
        author: "ReVanced Team",
        description: "YouTube ReVanced Module",
        mode: "magic" as const,
        is_mounted: true,
        enabled: true,
        blacklisted: false,
        source_path: "/data/adb/modules/youtube-revanced",
        mount_error: null,
        suggest_ignore: false,
        rules: {
          default_mode: "magic",
          paths: { "system/etc/hosts": "overlay" },
        },
      },
      {
        id: "sound-enhancer",
        name: "Sound Enhancer",
        version: "1.0",
        author: "AudioMod",
        description: "Improves system audio quality.",
        mode: "ignore" as const,
        is_mounted: false,
        enabled: false,
        blacklisted: false,
        source_path: "/data/adb/modules/sound-enhancer",
        mount_error: "mount_error marker present",
        suggest_ignore: true,
        rules: { default_mode: "ignore", paths: {} },
      },
      {
        id: "hosts-redirect",
        name: "Hosts Redirect",
        version: "2.3",
        author: "Demo",
        description: "Overlay hosts file module.",
        mode: "magic" as const,
        is_mounted: true,
        enabled: true,
        blacklisted: false,
        source_path: "/data/adb/modules/hosts-redirect",
        mount_error: null,
        suggest_ignore: false,
        rules: { default_mode: "magic", paths: {} },
      },
    ];
  },

  getStatus: async () => {
    await delay(MOCK_DELAY);
    return {
      timestamp: Math.floor(Date.now() / 1000),
      pid: 1,
      // Magic-only boot: no Tmpfs/Ext4 staging backend was created.
      storage_mode: "none",
      mount_point: "/data/adb/hybrid-mount/run",
      overlay_modules: [],
      magic_modules: ["youtube-revanced", "hosts-redirect"],
      skip_mount_modules: ["sound-enhancer"],
      active_mounts: [
        "/system/etc/hosts",
        "/system/framework/services.jar",
        "/vendor/etc/audio_effects.xml",
      ],
      overlay_active_mounts: [],
      magic_active_mounts: [
        "/system/etc/hosts",
        "/system/framework/services.jar",
        "/vendor/etc/audio_effects.xml",
      ],
      vfs_modules: [],
      vfs_active_mounts: [],
      vfs_provider: null,
      vfs_error: MOCK_STATUS_ERROR
        ? "VFS read-back mismatch: rule not reported by the provider"
        : null,
      vfs_error_modules: MOCK_STATUS_ERROR ? ["hosts-redirect"] : [],
      vfs_foreign_nomount: false,
      confirmed_active_mounts: [
        "/system/etc/hosts",
        "/system/framework/services.jar",
        "/vendor/etc/audio_effects.xml",
      ],
      mount_error_modules: MOCK_STATUS_ERROR
        ? ["sound-enhancer", "youtube-revanced"]
        : ["sound-enhancer"],
      mount_error_reasons: {
        "sound-enhancer": "mount_error marker present",
        ...(MOCK_STATUS_ERROR
          ? { "youtube-revanced": "overlay staging failed: ENOSPC" }
          : {}),
      } as Record<string, string>,
      mount_stats: {
        total_mounts: 4,
        successful_mounts: 4,
        failed_mounts: 0,
        files_mounted: 3,
        symlinks_created: 1,
        overlayfs_mounts: 0,
        ignored_entries: 0,
      },
      mode_stats: { overlayfs: 0, magicmount: 2, vfs: 0 },
      state_load: { kind: "loaded", detail: null },
      failed_stage: MOCK_STATUS_ERROR ? "magic_mount" : null,
      failure_reason: MOCK_STATUS_ERROR
        ? "bind mount /system/framework/services.jar failed: EPERM"
        : null,
      rollback_status: MOCK_STATUS_ERROR ? "incomplete" : "committed",
      leftover_mount_targets: MOCK_STATUS_ERROR ? ["/system/framework/services.jar"] : [],
    };
  },

  getInstallState: async () => {
    await delay(MOCK_DELAY);
    return {
      installed: true,
      self_module: true,
      binary: true,
      config_exists: true,
      overlay_supported: true,
      tmpfs_supported: true,
      nuke_supported: null,
      nuke_type: "apatch",
      vfs_supported: true,
      vfs_type: "lkm",
      mount_source: "KSU",
      compatible: true,
    };
  },

  clearMountErrors: async () => {
    await delay(MOCK_DELAY);
    console.log("[MockAPI] clearMountErrors");
    return 1;
  },

  getSystemInfo: async () => {
    await delay(MOCK_DELAY);
    return {
      kernel: "5.10.101-android12-9-00001-g532145",
      selinux: "Enforcing",
    };
  },

  getDeviceStatus: async () => {
    await delay(MOCK_DELAY);
    return { model: "Pixel 8 Pro (Mock)", android: "14", sdk: "34" };
  },

  getVersion: async () => {
    await delay(MOCK_DELAY);
    return "6.0.0-mock";
  },

  openLink: async (url: string) => {
    console.log("[MockAPI] openLink:", url);
    window.open(url, "_blank");
  },

  reboot: async () => {
    console.log("[MockAPI] reboot");
  },
};
