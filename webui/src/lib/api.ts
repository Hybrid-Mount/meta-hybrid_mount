// SPDX-License-Identifier: Apache-2.0

import type {
  AppAPI,
  AppConfig,
  InstallState,
  Module,
  ModuleRule,
  MountMode,
  OverlayMode,
  RunState,
  SystemInfo,
  DeviceInfo,
} from "./types";
import { MockAPI } from "./api.mock";
import { DEFAULT_CONFIG, PATHS } from "./constants";

interface KsuExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

type KsuExec = (cmd: string) => Promise<KsuExecResult>;

let ksuExec: KsuExec | null = null;

try {
  const ksu = await import("kernelsu").catch(() => null);
  ksuExec = ksu ? (ksu.exec as KsuExec) : null;
} catch {
  ksuExec = null;
}

const shouldUseMock = import.meta.env.DEV || !ksuExec;

function stringToHex(str: string): string {
  const bytes = new TextEncoder().encode(str);
  let hex = "";
  for (const byte of bytes) {
    hex += byte.toString(16).padStart(2, "0");
  }
  return hex;
}

const isMountMode = (value: unknown): value is MountMode =>
  value === "overlay" || value === "magic" || value === "ignore";

const isOverlayMode = (value: unknown): value is OverlayMode =>
  value === "tmpfs" || value === "ext4";

export function normalizeConfigPayload(payload: Record<string, unknown>): AppConfig {
  const rules: AppConfig["rules"] = {};

  if (payload.rules && typeof payload.rules === "object") {
    for (const [moduleId, rawRule] of Object.entries(
      payload.rules as Record<string, unknown>,
    )) {
      if (!rawRule || typeof rawRule !== "object") continue;

      const raw = rawRule as Record<string, unknown>;
      const defaultMode = isMountMode(raw.default_mode) ? raw.default_mode : null;
      const paths: Record<string, MountMode> = {};

      if (raw.paths && typeof raw.paths === "object") {
        for (const [path, mode] of Object.entries(raw.paths as Record<string, unknown>)) {
          if (isMountMode(mode)) {
            paths[path] = mode;
          }
        }
      }

      rules[moduleId] = { default_mode: defaultMode, paths };
    }
  }

  return {
    moduledir:
      typeof payload.moduledir === "string"
        ? payload.moduledir
        : DEFAULT_CONFIG.moduledir,
    mountsource:
      typeof payload.mountsource === "string"
        ? payload.mountsource
        : DEFAULT_CONFIG.mountsource,
    overlay_mode: isOverlayMode(payload.overlay_mode)
      ? payload.overlay_mode
      : DEFAULT_CONFIG.overlay_mode,
    disable_umount:
      typeof payload.disable_umount === "boolean"
        ? payload.disable_umount
        : DEFAULT_CONFIG.disable_umount,
    default_mode: isMountMode(payload.default_mode)
      ? payload.default_mode
      : DEFAULT_CONFIG.default_mode,
    rules,
  };
}

export function createConfigPayload(config: AppConfig): Record<string, unknown> {
  return {
    moduledir: config.moduledir,
    mountsource: config.mountsource,
    overlay_mode: config.overlay_mode,
    disable_umount: config.disable_umount,
    default_mode: config.default_mode,
    replace_rules: true,
    rules: config.rules,
  };
}

export function normalizeModule(raw: Record<string, unknown>): Module {
  const rawRules =
    raw.rules && typeof raw.rules === "object"
      ? (raw.rules as Record<string, unknown>)
      : {};

  const paths: Record<string, MountMode> = {};
  if (rawRules.paths && typeof rawRules.paths === "object") {
    for (const [path, mode] of Object.entries(
      rawRules.paths as Record<string, unknown>,
    )) {
      if (isMountMode(mode)) paths[path] = mode;
    }
  }

  return {
    id: String(raw.id ?? ""),
    name: String(raw.name ?? raw.id ?? "Unknown"),
    version: String(raw.version ?? ""),
    author: String(raw.author ?? "Unknown"),
    description: String(raw.description ?? ""),
    mode: isMountMode(raw.mode) ? raw.mode : "ignore",
    is_mounted: Boolean(raw.is_mounted),
    enabled: typeof raw.enabled === "boolean" ? raw.enabled : true,
    source_path: String(raw.source_path ?? ""),
    mount_error:
      typeof raw.mount_error === "string" && raw.mount_error.length > 0
        ? raw.mount_error
        : null,
    suggest_ignore: Boolean(raw.suggest_ignore),
    rules: {
      default_mode: isMountMode(rawRules.default_mode) ? rawRules.default_mode : null,
      paths,
    },
  };
}

function normalizeStatus(payload: Record<string, unknown>): RunState {
  return {
    timestamp: Number(payload.timestamp ?? 0),
    pid: Number(payload.pid ?? 0),
    storage_mode: String(payload.storage_mode ?? "ext4"),
    mount_point: String(payload.mount_point ?? ""),
    overlay_modules: Array.isArray(payload.overlay_modules)
      ? payload.overlay_modules.map(String)
      : [],
    magic_modules: Array.isArray(payload.magic_modules)
      ? payload.magic_modules.map(String)
      : [],
    skip_mount_modules: Array.isArray(payload.skip_mount_modules)
      ? payload.skip_mount_modules.map(String)
      : [],
    active_mounts: Array.isArray(payload.active_mounts)
      ? payload.active_mounts.map(String)
      : [],
    mount_error_modules: Array.isArray(payload.mount_error_modules)
      ? payload.mount_error_modules.map(String)
      : [],
    mount_error_reasons:
      payload.mount_error_reasons && typeof payload.mount_error_reasons === "object"
        ? (payload.mount_error_reasons as Record<string, string>)
        : {},
    mount_stats: {
      total_mounts: Number(
        (payload.mount_stats as Record<string, unknown>)?.total_mounts ?? 0,
      ),
      successful_mounts: Number(
        (payload.mount_stats as Record<string, unknown>)?.successful_mounts ?? 0,
      ),
      failed_mounts: Number(
        (payload.mount_stats as Record<string, unknown>)?.failed_mounts ?? 0,
      ),
      files_mounted: Number(
        (payload.mount_stats as Record<string, unknown>)?.files_mounted ?? 0,
      ),
      symlinks_created: Number(
        (payload.mount_stats as Record<string, unknown>)?.symlinks_created ?? 0,
      ),
      overlayfs_mounts: Number(
        (payload.mount_stats as Record<string, unknown>)?.overlayfs_mounts ?? 0,
      ),
      ignored_entries: Number(
        (payload.mount_stats as Record<string, unknown>)?.ignored_entries ?? 0,
      ),
    },
    mode_stats: {
      overlayfs: Number((payload.mode_stats as Record<string, unknown>)?.overlayfs ?? 0),
      magicmount: Number(
        (payload.mode_stats as Record<string, unknown>)?.magicmount ?? 0,
      ),
    },
  };
}

function normalizeInstallState(payload: Record<string, unknown>): InstallState {
  return {
    installed: Boolean(payload.installed),
    self_module: Boolean(payload.self_module),
    binary: Boolean(payload.binary),
    config_exists: Boolean(payload.config_exists),
    overlay_supported: Boolean(payload.overlay_supported),
    mount_source: String(payload.mount_source ?? "unknown"),
    compatible: Boolean(payload.compatible),
  };
}

const shellEscapeDoubleQuoted = (value: string): string =>
  value.replace(/(["\\$`])/g, "\\$1");

const RealAPI: AppAPI = {
  loadConfig: async () => {
    const { errno, stdout, stderr } = await ksuExec!(`${PATHS.BINARY} show-config`);
    if (errno === 0 && stdout.trim()) {
      return normalizeConfigPayload(JSON.parse(stdout));
    }
    throw new Error(stderr || "show-config failed");
  },

  saveConfig: async (config: AppConfig) => {
    const payload = stringToHex(JSON.stringify(createConfigPayload(config)));
    const { errno, stderr } = await ksuExec!(
      `${PATHS.BINARY} save-config --payload ${payload}`,
    );
    if (errno !== 0) throw new Error(stderr || "save-config failed");
  },

  genConfig: async () => {
    const { errno, stderr } = await ksuExec!(`${PATHS.BINARY} gen-config`);
    if (errno !== 0) throw new Error(stderr || "gen-config failed");
  },

  saveModuleRules: async (moduleId: string, rules: ModuleRule) => {
    const payload = stringToHex(JSON.stringify({ rules: { [moduleId]: rules } }));
    const { errno, stderr } = await ksuExec!(
      `${PATHS.BINARY} save-config --payload ${payload}`,
    );
    if (errno !== 0) throw new Error(stderr || "save module rules failed");
  },

  scanModules: async () => {
    const { errno, stdout, stderr } = await ksuExec!(`${PATHS.BINARY} modules`);
    if (errno === 0 && stdout) {
      return (JSON.parse(stdout) as Record<string, unknown>[]).map(normalizeModule);
    }
    throw new Error(stderr || "modules failed");
  },

  getStatus: async () => {
    const { errno, stdout, stderr } = await ksuExec!(`${PATHS.BINARY} status`);
    if (errno === 0 && stdout) {
      return normalizeStatus(JSON.parse(stdout));
    }
    throw new Error(stderr || "status failed");
  },

  getInstallState: async () => {
    const { errno, stdout, stderr } = await ksuExec!(`${PATHS.BINARY} install-state`);
    if (errno === 0 && stdout) {
      return normalizeInstallState(JSON.parse(stdout));
    }
    throw new Error(stderr || "install-state failed");
  },

  clearMountErrors: async () => {
    const { errno, stdout, stderr } = await ksuExec!(
      `${PATHS.BINARY} clear-mount-errors`,
    );
    if (errno === 0 && stdout) {
      const result = JSON.parse(stdout) as { ok?: boolean; removed?: unknown };
      return Number(result.removed ?? 0);
    }
    throw new Error(stderr || "clear-mount-errors failed");
  },

  getSystemInfo: async () => {
    const info: SystemInfo = { kernel: "-", selinux: "-" };
    try {
      const cmd = `echo "KERNEL:$(uname -r)"\necho "SELINUX:$(getenforce)"`;
      const { errno, stdout } = await ksuExec!(cmd);
      if (errno === 0 && stdout) {
        for (const line of stdout.split("\n")) {
          if (line.startsWith("KERNEL:")) info.kernel = line.slice(7).trim();
          else if (line.startsWith("SELINUX:")) info.selinux = line.slice(8).trim();
        }
      }
    } catch {
      // best effort
    }
    return info;
  },

  getDeviceStatus: async () => {
    const info: DeviceInfo = { model: "-", android: "-", sdk: "-" };
    try {
      const cmd =
        "getprop ro.product.model\ngetprop ro.build.version.release\ngetprop ro.build.version.sdk";
      const { errno, stdout } = await ksuExec!(cmd);
      if (errno === 0 && stdout) {
        const lines = stdout.split("\n");
        info.model = lines[0]?.trim() || "-";
        info.android = lines[1]?.trim() || "-";
        info.sdk = lines[2]?.trim() || "-";
      }
    } catch {
      // best effort
    }
    return info;
  },

  getVersion: async () => {
    try {
      const { errno, stdout } = await ksuExec!(`${PATHS.BINARY} version`);
      if (errno === 0 && stdout) {
        const result = JSON.parse(stdout) as { version?: string };
        return result.version ?? stdout.trim();
      }
    } catch {
      // fall through
    }
    return "Unknown";
  },

  openLink: async (url: string) => {
    const safeUrl = shellEscapeDoubleQuoted(url);
    await ksuExec!(`am start -a android.intent.action.VIEW -d "${safeUrl}"`);
  },

  reboot: async () => {
    const debug = await ksuExec!('ksud debug info | grep "late_load: "');
    const lateLoad = debug.errno === 0 && debug.stdout.slice(11).trim() === "true";
    const result = await ksuExec!(
      lateLoad ? "ksud soft-reboot" : "svc power reboot || reboot",
    );
    if (result.errno !== 0) {
      throw new Error(result.stderr || "reboot command failed");
    }
  },
};

export const API: AppAPI = shouldUseMock ? MockAPI : RealAPI;
