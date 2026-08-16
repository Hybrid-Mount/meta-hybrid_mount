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

export interface AppConfig {
  moduledir: string;
  mountsource: string;
  overlay_mode: OverlayMode;
  disable_umount: boolean;
  default_mode: MountMode;
  daemon_startup_mode: "on-demand" | "persistent";
  rules: Record<string, ModuleRules>;
}

export type MountMode = "overlay" | "magic" | "ignore";

export interface Module {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  mode: MountMode;
  is_mounted: boolean;
  enabled?: boolean;
  source_path?: string;
  rules: ModuleRules;
  mount_error?: string;
  suggest_ignore?: boolean;
}

export interface StorageStatus {
  type: "tmpfs" | "ext4" | "unknown" | null;
  error?: string;
  supported_modes?: OverlayMode[];
  modeStats?: ModeStats;
  mountedCount?: number;
}

export interface SystemInfo {
  kernel: string;
  selinux: string;
  mountBase: string;
  activeMounts: string[];
  supported_overlay_modes?: OverlayMode[];
  tmpfs_xattr_supported?: boolean;
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
  blacklisted: number;
}
