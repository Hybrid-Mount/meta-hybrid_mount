import { DEFAULT_CONFIG } from "../../constants";
import { ENABLE_KASUMI } from "../../constants_gen";
import type {
  AppConfig,
  KasumiConfig,
  KasumiUnameConfig,
  ModuleRules,
  MountMode,
  OverlayMode,
} from "../../types";
import {
  isBoolean,
  isRecord,
  isString,
  normalizeMountMode as normalizeMountModeBase,
  toNonNegativeInt,
} from "../core/guards";

export function normalizeMountMode(
  value: unknown,
  fallback: MountMode = "overlay",
): MountMode {
  return normalizeMountModeBase(value, fallback);
}

export function normalizeOverlayMode(value: unknown): OverlayMode {
  return value === "tmpfs" ? "tmpfs" : "ext4";
}

export function normalizeStringMap(value: unknown): Record<string, string> {
  if (!isRecord(value)) return {};
  const result: Record<string, string> = {};
  for (const [key, entry] of Object.entries(value)) {
    if (isString(entry)) {
      result[key] = normalizeMountMode(entry);
    }
  }
  return result;
}

export function normalizeKasumiUname(value: unknown): KasumiUnameConfig {
  const next = isRecord(value) ? value : {};
  return {
    sysname: isString(next.sysname) ? next.sysname : "",
    nodename: isString(next.nodename) ? next.nodename : "",
    release: isString(next.release) ? next.release : "",
    version: isString(next.version) ? next.version : "",
    machine: isString(next.machine) ? next.machine : "",
    domainname: isString(next.domainname) ? next.domainname : "",
  };
}

export function normalizeKasumiUnameMode(
  value: unknown,
): AppConfig["kasumi"]["uname_mode"] {
  return value === "global" ? "global" : "scoped";
}

export function normalizeKasumiConfig(value: unknown): KasumiConfig {
  const next = isRecord(value) ? value : {};
  const mountHide = isRecord(next.mount_hide) ? next.mount_hide : {};
  const statfsSpoof = isRecord(next.statfs_spoof) ? next.statfs_spoof : {};
  const uname = normalizeKasumiUname(next.uname);

  return {
    enabled: isBoolean(next.enabled)
      ? next.enabled
      : DEFAULT_CONFIG.kasumi.enabled,
    lkm_autoload: isBoolean(next.lkm_autoload)
      ? next.lkm_autoload
      : DEFAULT_CONFIG.kasumi.lkm_autoload,
    lkm_dir: isString(next.lkm_dir)
      ? next.lkm_dir
      : DEFAULT_CONFIG.kasumi.lkm_dir,
    lkm_kmi_override: isString(next.lkm_kmi_override)
      ? next.lkm_kmi_override
      : DEFAULT_CONFIG.kasumi.lkm_kmi_override,
    mirror_path: isString(next.mirror_path)
      ? next.mirror_path
      : DEFAULT_CONFIG.kasumi.mirror_path,
    enable_kernel_debug: isBoolean(next.enable_kernel_debug)
      ? next.enable_kernel_debug
      : DEFAULT_CONFIG.kasumi.enable_kernel_debug,
    enable_stealth: isBoolean(next.enable_stealth)
      ? next.enable_stealth
      : DEFAULT_CONFIG.kasumi.enable_stealth,
    enable_hidexattr: isBoolean(next.enable_hidexattr)
      ? next.enable_hidexattr
      : DEFAULT_CONFIG.kasumi.enable_hidexattr,
    enable_selinux_fix: isBoolean(next.enable_selinux_fix)
      ? next.enable_selinux_fix
      : DEFAULT_CONFIG.kasumi.enable_selinux_fix,
    enable_mount_hide: isBoolean(next.enable_mount_hide)
      ? next.enable_mount_hide
      : DEFAULT_CONFIG.kasumi.enable_mount_hide,
    enable_maps_spoof: isBoolean(next.enable_maps_spoof)
      ? next.enable_maps_spoof
      : DEFAULT_CONFIG.kasumi.enable_maps_spoof,
    enable_statfs_spoof: isBoolean(next.enable_statfs_spoof)
      ? next.enable_statfs_spoof
      : DEFAULT_CONFIG.kasumi.enable_statfs_spoof,
    mount_hide: {
      enabled: isBoolean(mountHide.enabled) ? mountHide.enabled : false,
      path_pattern: isString(mountHide.path_pattern)
        ? mountHide.path_pattern
        : "",
    },
    statfs_spoof: {
      enabled: isBoolean(statfsSpoof.enabled) ? statfsSpoof.enabled : false,
      path: isString(statfsSpoof.path) ? statfsSpoof.path : "",
      spoof_f_type: toNonNegativeInt(statfsSpoof.spoof_f_type),
    },
    hide_uids: Array.isArray(next.hide_uids)
      ? next.hide_uids
          .map((item) => toNonNegativeInt(item))
          .filter((item) => item >= 0)
      : [],
    uname_mode: normalizeKasumiUnameMode(next.uname_mode),
    uname,
    cmdline_value: isString(next.cmdline_value) ? next.cmdline_value : "",
    kstat_rules: Array.isArray(next.kstat_rules)
      ? next.kstat_rules.filter(isRecord).map((item) => ({
          target_ino: toNonNegativeInt(item.target_ino),
          target_pathname: isString(item.target_pathname)
            ? item.target_pathname
            : "",
          spoofed_ino: toNonNegativeInt(item.spoofed_ino),
          spoofed_dev: toNonNegativeInt(item.spoofed_dev),
          spoofed_nlink: toNonNegativeInt(item.spoofed_nlink),
          spoofed_size: Number(item.spoofed_size || 0),
          spoofed_atime_sec: Number(item.spoofed_atime_sec || 0),
          spoofed_atime_nsec: Number(item.spoofed_atime_nsec || 0),
          spoofed_mtime_sec: Number(item.spoofed_mtime_sec || 0),
          spoofed_mtime_nsec: Number(item.spoofed_mtime_nsec || 0),
          spoofed_ctime_sec: Number(item.spoofed_ctime_sec || 0),
          spoofed_ctime_nsec: Number(item.spoofed_ctime_nsec || 0),
          spoofed_blksize: toNonNegativeInt(item.spoofed_blksize),
          spoofed_blocks: toNonNegativeInt(item.spoofed_blocks),
          is_static: isBoolean(item.is_static) ? item.is_static : false,
        }))
      : [],
    maps_rules: Array.isArray(next.maps_rules)
      ? next.maps_rules.filter(isRecord).map((item) => ({
          target_ino: toNonNegativeInt(item.target_ino),
          target_dev: toNonNegativeInt(item.target_dev),
          spoofed_ino: toNonNegativeInt(item.spoofed_ino),
          spoofed_dev: toNonNegativeInt(item.spoofed_dev),
          spoofed_pathname: isString(item.spoofed_pathname)
            ? item.spoofed_pathname
            : "",
        }))
      : [],
  };
}

export function normalizeConfig(value: unknown): AppConfig {
  const next = isRecord(value) ? value : {};
  const defaultMode = normalizeMountMode(
    next.default_mode,
    DEFAULT_CONFIG.default_mode,
  );
  const rulesSource = isRecord(next.rules) ? next.rules : {};
  const rules: Record<string, ModuleRules> = {};

  for (const [moduleId, ruleValue] of Object.entries(rulesSource)) {
    if (!isRecord(ruleValue)) continue;
    rules[moduleId] = {
      default_mode: normalizeMountMode(ruleValue.default_mode, defaultMode),
      paths: normalizeStringMap(ruleValue.paths),
    };
  }

  const normalized = {
    moduledir: isString(next.moduledir)
      ? next.moduledir
      : DEFAULT_CONFIG.moduledir,
    mountsource: isString(next.mountsource)
      ? next.mountsource
      : DEFAULT_CONFIG.mountsource,
    overlay_mode: normalizeOverlayMode(next.overlay_mode),
    disable_umount: isBoolean(next.disable_umount)
      ? next.disable_umount
      : DEFAULT_CONFIG.disable_umount,
    default_mode: defaultMode,
    daemon_startup_mode:
      next.daemon_startup_mode === "persistent" ? "persistent" : "on-demand",
    rules,
  };

  return {
    ...normalized,
    ...(ENABLE_KASUMI ? { kasumi: normalizeKasumiConfig(next.kasumi) } : {}),
  } as AppConfig;
}
