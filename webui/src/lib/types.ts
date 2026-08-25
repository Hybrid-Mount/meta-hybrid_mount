// SPDX-License-Identifier: Apache-2.0

export type MountMode = "overlay" | "magic" | "ignore";
export type DefaultMountMode = Exclude<MountMode, "ignore">;
export type OverlayMode = "tmpfs" | "ext4";
export type UiStyle = "miuix" | "md3";

export interface ModuleRule {
  default_mode: MountMode | null;
  paths: Record<string, MountMode>;
}

export interface AppConfig {
  moduledir: string;
  mountsource: string;
  overlay_mode: OverlayMode;
  tmpfs_xattr_supported: boolean;
  disable_umount: boolean;
  default_mode: DefaultMountMode;
  rules: Record<string, ModuleRule>;
}

export interface ModuleRulesView {
  default_mode: MountMode | null;
  paths: Record<string, MountMode>;
}

export interface Module {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  mode: MountMode;
  is_mounted: boolean;
  enabled: boolean;
  source_path: string;
  mount_error: string | null;
  suggest_ignore: boolean;
  rules: ModuleRulesView;
}

export interface MountStatistics {
  total_mounts: number;
  successful_mounts: number;
  failed_mounts: number;
  files_mounted: number;
  symlinks_created: number;
  overlayfs_mounts: number;
  ignored_entries: number;
}

export interface ModeStats {
  overlayfs: number;
  magicmount: number;
}

export interface RunState {
  timestamp: number;
  pid: number;
  storage_mode: string;
  mount_point: string;
  overlay_modules: string[];
  magic_modules: string[];
  skip_mount_modules: string[];
  active_mounts: string[];
  mount_error_modules: string[];
  mount_error_reasons: Record<string, string>;
  mount_stats: MountStatistics;
  mode_stats: ModeStats;
}

export interface InstallState {
  installed: boolean;
  self_module: boolean;
  binary: boolean;
  config_exists: boolean;
  overlay_supported: boolean;
  mount_source: string;
  compatible: boolean;
}

export interface SystemInfo {
  kernel: string;
  selinux: string;
}

export interface DeviceInfo {
  model: string;
  android: string;
  sdk: string;
}

export interface AppAPI {
  loadConfig: () => Promise<AppConfig>;
  saveConfig: (config: AppConfig) => Promise<void>;
  genConfig: () => Promise<void>;
  saveModuleRules: (moduleId: string, rules: ModuleRule) => Promise<void>;
  scanModules: () => Promise<Module[]>;
  getStatus: () => Promise<RunState>;
  getInstallState: () => Promise<InstallState>;
  clearMountErrors: () => Promise<number>;
  getSystemInfo: () => Promise<SystemInfo>;
  getDeviceStatus: () => Promise<DeviceInfo>;
  getVersion: () => Promise<string>;
  openLink: (url: string) => Promise<void>;
  reboot: () => Promise<void>;
}
