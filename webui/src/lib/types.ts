/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

export interface ModuleRules {
  default_mode: MountMode;
  paths: Record<string, string>;
}

export type OverlayMode = "tmpfs" | "ext4" | "erofs";

export interface AppConfig {
  moduledir: string;
  mountsource: string;
  verbose: boolean;
  hybrid_mnt_dir: string;
  partitions: string[];
  overlay_mode: OverlayMode;
  disable_umount: boolean;
  allow_umount_coexistence: boolean;
  logfile?: string;
}

export type MountMode = "overlay" | "magic" | "ignore";

export interface Module {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  mode: string;
  is_mounted: boolean;
  enabled?: boolean;
  source_path?: string;
  rules: ModuleRules;
}

export interface StorageStatus {
  size: string;
  used: string;
  percent: string;
  type: "tmpfs" | "ext4" | "erofs" | "unknown" | null;
  error?: string;
  hymofs_available?: boolean;
}

export interface SystemInfo {
  kernel: string;
  selinux: string;
  mountBase: string;
  activeMounts: string[];
  zygisksuEnforce?: string;
  supported_overlay_modes?: OverlayMode[];
}

export interface DeviceInfo {
  model: string;
  android: string;
  kernel: string;
  selinux: string;
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
  auto: number;
  magic: number;
}

export interface ConflictEntry {
  partition: string;
  relative_path: string;
  contending_modules: string[];
}

export interface DiagnosticIssue {
  level: "Info" | "Warning" | "Critical";
  context: string;
  message: string;
}
