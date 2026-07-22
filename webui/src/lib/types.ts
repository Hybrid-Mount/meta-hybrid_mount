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

export interface ModuleRules {
  default_mode: MountMode;
  paths: Record<string, string>;
}

export type OverlayMode = "tmpfs" | "ext4";

export interface CustomBindMount {
  source: string;
  target: string;
}

export interface AppConfig {
  moduledir: string;
  mountsource: string;
  overlay_mode: OverlayMode;
  disable_umount: boolean;
  default_mode: MountMode;
  custom_mounts: CustomBindMount[];
  kasumi: KasumiConfig;
  rules: Record<string, ModuleRules>;
}

export type MountMode = "overlay" | "magic" | "kasumi" | "ignore";

export interface Module {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  mode: MountMode;
  is_mounted: boolean;
  enabled: boolean;
  rules: ModuleRules;
  is_blacklisted: boolean;
}

export interface StorageStatus {
  type: OverlayMode;
  modeStats: ModeStats;
  mountedCount: number;
}

export interface SystemInfo {
  kernel: string;
  selinux: string;
  mountBase: string;
  activeMounts: string[];
  supported_overlay_modes: OverlayMode[];
  tmpfs_xattr_supported: boolean;
}

export interface KasumiLkmStatus {
  loaded: boolean;
  module_name: string | null;
  autoload: boolean;
  kmi_override: string;
  current_kmi: string;
  search_dir: string;
  module_file: string;
}

export interface KasumiUnameConfig {
  sysname: string;
  nodename: string;
  release: string;
  version: string;
  machine: string;
  domainname: string;
}

export type KasumiUnameMode = "scoped" | "global";

export interface KernelUnameValues {
  release: string;
  version: string;
}

export interface KasumiMountHideConfig {
  enabled: boolean;
  path_pattern: string;
}

export interface KasumiStatfsSpoofConfig {
  enabled: boolean;
  path: string;
  spoof_f_type: number;
}

export interface KasumiMapsRuleConfig {
  target_ino: number;
  target_dev: number;
  spoofed_ino: number;
  spoofed_dev: number;
  spoofed_pathname: string;
}

export interface KasumiKstatRuleConfig {
  target_ino: number;
  target_pathname: string;
  spoofed_ino: number;
  spoofed_dev: number;
  spoofed_nlink: number;
  spoofed_size: number;
  spoofed_atime_sec: number;
  spoofed_atime_nsec: number;
  spoofed_mtime_sec: number;
  spoofed_mtime_nsec: number;
  spoofed_ctime_sec: number;
  spoofed_ctime_nsec: number;
  spoofed_blksize: number;
  spoofed_blocks: number;
  is_static: boolean;
}

export interface KasumiConfig {
  enabled: boolean;
  lkm_autoload: boolean;
  lkm_dir: string;
  lkm_kmi_override: string;
  mirror_path: string;
  enable_kernel_debug: boolean;
  enable_stealth: boolean;
  enable_overlay_xattr_hide: boolean;
  enable_selinux_fix: boolean;
  enable_mount_hide: boolean;
  enable_maps_spoof: boolean;
  enable_statfs_spoof: boolean;
  mount_hide: KasumiMountHideConfig;
  statfs_spoof: KasumiStatfsSpoofConfig;
  hide_uids: number[];
  uname_mode: KasumiUnameMode;
  uname: KasumiUnameConfig;
  cmdline_value: string;
  kstat_rules: KasumiKstatRuleConfig[];
  maps_rules: KasumiMapsRuleConfig[];
}

export interface KasumiRuntimeInfo {
  snapshot: KasumiRuntimeSnapshot;
  kasumi_modules: string[];
  active_mounts: string[];
}

export interface KasumiRuntimeSnapshot {
  status: string;
  available: boolean;
  kernel_supported: boolean;
  lkm_loaded: boolean;
  lkm_autoload: boolean;
  lkm_kmi_override: string;
  lkm_current_kmi: string;
  lkm_dir: string;
  protocol_version: number | null;
  feature_bits: number | null;
  feature_names: string[];
  hooks: string[];
  rule_count: number;
  user_hide_rule_count: number;
  mirror_path: string;
}

export interface KasumiStatus {
  status: string;
  available: boolean;
  kernel_supported: boolean;
  protocol_version: number | null;
  feature_bits: number | null;
  feature_names: string[];
  hooks: string[];
  rule_count: number;
  user_hide_rule_count: number;
  mirror_path: string;
  lkm: KasumiLkmStatus;
  config: KasumiConfig;
  runtime: KasumiRuntimeInfo;
}

export interface ToastMessage {
  id: string;
  text: string;
  type: "info" | "success" | "error";
  visible: boolean;
}

export interface LanguageOption {
  code: string;
  name: string;
  display?: string;
}

export interface ModeStats {
  overlay: number;
  magic: number;
  kasumi: number;
  blacklisted: number;
}
