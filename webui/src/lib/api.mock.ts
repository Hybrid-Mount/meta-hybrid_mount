// SPDX-License-Identifier: Apache-2.0

import type { AppAPI, AppConfig, Module, ModuleRule } from "./types";
import { DEFAULT_CONFIG } from "./constants";

const MOCK_DELAY = 300;
const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

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
      storage_mode: "ext4",
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
      mount_error_modules: ["sound-enhancer"],
      mount_error_reasons: {
        "sound-enhancer": "mount_error marker present",
      },
      mount_stats: {
        total_mounts: 4,
        successful_mounts: 4,
        failed_mounts: 0,
        files_mounted: 3,
        symlinks_created: 1,
        overlayfs_mounts: 0,
        ignored_entries: 0,
      },
      mode_stats: { overlayfs: 0, magicmount: 2 },
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
