// SPDX-License-Identifier: Apache-2.0

import type { AppAPI, AppConfig, Module, ModuleRule, RuntimeStatus } from "./types";
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

const runtime: RuntimeStatus = {
  supported: import.meta.env.VITE_MOCK_RUNTIME_UNSUPPORTED !== "1",
  reason:
    import.meta.env.VITE_MOCK_RUNTIME_UNSUPPORTED === "1"
      ? "No trusted runtime ledger for this boot"
      : null,
  generation: 1,
  modules: [
    {
      id: "youtube-revanced",
      active: false,
      eligible: false,
      reason: "Module has active Magic Mount mappings",
    },
    { id: "sound-enhancer", active: false, eligible: true, reason: null },
    { id: "hosts-redirect", active: true, eligible: true, reason: null },
  ],
};
const runtimeActive = (id: string) =>
  runtime.modules.some((item) => item.id === id && item.active);

export const MockAPI: AppAPI = {
  getRuntimeStatus: async () => {
    await delay(MOCK_DELAY);
    return structuredClone(runtime);
  },
  runtimeAction: async (moduleId, action) => {
    await delay(MOCK_DELAY);
    if (import.meta.env.VITE_MOCK_RUNTIME_ERROR === "1")
      throw new Error("Provider rejected the runtime update");
    const module = runtime.modules.find((item) => item.id === moduleId);
    if (!runtime.supported || !module?.eligible)
      throw new Error(module?.reason || runtime.reason || "Module unavailable");
    module.active = action !== "unload";
    runtime.generation += 1;
    return { ok: true, generation: runtime.generation };
  },
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
        is_mounted: runtimeActive("sound-enhancer"),
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
        description: "VFS hosts file module.",
        mode: "vfs" as const,
        is_mounted: runtimeActive("hosts-redirect"),
        enabled: true,
        blacklisted: false,
        source_path: "/data/adb/modules/hosts-redirect",
        mount_error: null,
        suggest_ignore: false,
        rules: { default_mode: "vfs", paths: {} },
      },
    ];
  },

  getStatus: async () => {
    await delay(MOCK_DELAY);
    const vfsMounts = [
      ...(runtimeActive("hosts-redirect") ? ["/system/etc/hosts"] : []),
      ...(runtimeActive("sound-enhancer") ? ["/vendor/etc/audio_effects.xml"] : []),
    ];
    const activeMounts = ["/system/framework/services.jar", ...vfsMounts];
    return {
      timestamp: Math.floor(Date.now() / 1000),
      pid: 1,
      // No overlay staging backend was created.
      storage_mode: "none",
      mount_point: "/data/adb/hybrid-mount/run",
      overlay_modules: [],
      magic_modules: ["youtube-revanced"],
      skip_mount_modules: ["sound-enhancer"],
      active_mounts: activeMounts,
      overlay_active_mounts: [],
      magic_active_mounts: ["/system/framework/services.jar"],
      vfs_modules: runtime.modules.filter((item) => item.active).map((item) => item.id),
      vfs_active_mounts: vfsMounts,
      vfs_provider: "lkm",
      vfs_error: MOCK_STATUS_ERROR
        ? "VFS read-back mismatch: rule not reported by the provider"
        : null,
      vfs_error_modules: MOCK_STATUS_ERROR ? ["hosts-redirect"] : [],
      vfs_foreign_nomount: false,
      confirmed_active_mounts: activeMounts,
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
        total_mounts: activeMounts.length,
        successful_mounts: activeMounts.length,
        failed_mounts: 0,
        files_mounted: activeMounts.length,
        symlinks_created: 1,
        overlayfs_mounts: 0,
        ignored_entries: 0,
      },
      mode_stats: {
        overlayfs: 0,
        magicmount: 1,
        vfs: runtime.modules.filter((item) => item.active).length,
      },
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
