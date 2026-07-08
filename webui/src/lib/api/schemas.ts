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

// Zod schemas for all daemon API response payloads.
// Wire format stays unchanged; these provide runtime validation + compile-time type inference.
//
// Nested-object fields that may be missing from the wire payload use .optional().
// Individual scalar fields use .default() for per-field fallbacks.
// Complex normalisation (Kasumi config, module metadata, etc.) stays in the
// codec layer (configCodec.ts / runtimeCodec.ts), which already handles
// missing/null values with sensible defaults.

import { z } from "zod/v4";

// ── Primitives ───────────────────────────────────────────────────────────

export const mountModeSchema = z.enum(["overlay", "magic", "kasumi", "ignore"]);

export const overlayModeSchema = z.enum(["tmpfs", "ext4"]);

export const daemonStartupModeSchema = z.enum(["on-demand", "persistent"]);

export const kasumiUnameModeSchema = z.enum(["scoped", "global"]);

// ── Module rules ─────────────────────────────────────────────────────────

export const moduleRulesSchema = z.object({
  default_mode: mountModeSchema.default("overlay"),
  paths: z.record(z.string(), z.string()).default({}),
});

export type ModuleRulesPayload = z.infer<typeof moduleRulesSchema>;

// ── Module runtime entry (api-modules-list response item) ───────────────

export const moduleRuntimeEntrySchema = z.object({
  id: z.string(),
  name: z.string().optional(),
  version: z.string().optional(),
  author: z.string().optional(),
  description: z.string().optional(),
  mode: mountModeSchema.default("overlay"),
  is_mounted: z.boolean().default(false),
  enabled: z.boolean().default(true),
  source_path: z.string().optional(),
  rules: moduleRulesSchema.optional(),
  mount_error: z.string().optional(),
  suggest_ignore: z.boolean().optional(),
});

export type ModuleRuntimeEntryRaw = z.infer<typeof moduleRuntimeEntrySchema>;

// ── System info (api-system-info response) ──────────────────────────────

export const systemInfoSchema = z.object({
  kernel: z.string().default("Unknown"),
  selinux: z.string().default("Unknown"),
  mount_base: z.string().default("-"),
  active_mounts: z.array(z.string()).default([]),
  tmpfs_xattr_supported: z.boolean().optional(),
  supported_overlay_modes: z.array(z.string()).default(["tmpfs", "ext4"]),
});

export type SystemInfoPayload = z.infer<typeof systemInfoSchema>;

// ── Version (api-version response) ──────────────────────────────────────

export const versionSchema = z.object({
  version: z.string(),
});

// ── Kernel uname (api-kernel-uname response) ────────────────────────────

export const kernelUnameSchema = z.object({
  release: z.string(),
  version: z.string(),
});

// ── Init payload ─────────────────────────────────────────────────────────

export const initPayloadSchema = z.object({
  status: z.unknown(),
  config: z.unknown(),
  version: z.unknown(),
  kasumi_status: z.unknown().optional(),
  system_info: z.unknown(),
});

export type InitPayloadRaw = z.infer<typeof initPayloadSchema>;

// ── LKM status ──────────────────────────────────────────────────────────

export const kasumiLkmStatusSchema = z.object({
  loaded: z.boolean().default(false),
  module_name: z.string().optional(),
  autoload: z.boolean().default(false),
  kmi_override: z.string().default(""),
  current_kmi: z.string().default(""),
  search_dir: z.string().default(""),
  module_file: z.string().optional(),
  last_error: z.string().nullable().default(null),
});

export type KasumiLkmStatusPayload = z.infer<typeof kasumiLkmStatusSchema>;

// ── Kasumi uname config ─────────────────────────────────────────────────

export const kasumiUnameConfigSchema = z.object({
  sysname: z.string().default(""),
  nodename: z.string().default(""),
  release: z.string().default(""),
  version: z.string().default(""),
  machine: z.string().default(""),
  domainname: z.string().default(""),
});

export type KasumiUnameConfigPayload = z.infer<typeof kasumiUnameConfigSchema>;

// ── Kasumi sub-configs ──────────────────────────────────────────────────

export const kasumiMountHideConfigSchema = z.object({
  enabled: z.boolean().default(false),
  path_pattern: z.string().default(""),
});

export const kasumiStatfsSpoofConfigSchema = z.object({
  enabled: z.boolean().default(false),
  path: z.string().default(""),
  spoof_f_type: z.number().int().nonnegative().default(0),
});

export const kasumiMapsRuleConfigSchema = z.object({
  target_ino: z.number().int().nonnegative().default(0),
  target_dev: z.number().int().nonnegative().default(0),
  spoofed_ino: z.number().int().nonnegative().default(0),
  spoofed_dev: z.number().int().nonnegative().default(0),
  spoofed_pathname: z.string().default(""),
});

export const kasumiKstatRuleConfigSchema = z.object({
  target_ino: z.number().int().nonnegative().default(0),
  target_pathname: z.string().default(""),
  spoofed_ino: z.number().int().nonnegative().default(0),
  spoofed_dev: z.number().int().nonnegative().default(0),
  spoofed_nlink: z.number().int().nonnegative().default(0),
  spoofed_size: z.number().default(0),
  spoofed_atime_sec: z.number().default(0),
  spoofed_atime_nsec: z.number().default(0),
  spoofed_mtime_sec: z.number().default(0),
  spoofed_mtime_nsec: z.number().default(0),
  spoofed_ctime_sec: z.number().default(0),
  spoofed_ctime_nsec: z.number().default(0),
  spoofed_blksize: z.number().int().nonnegative().default(0),
  spoofed_blocks: z.number().int().nonnegative().default(0),
  is_static: z.boolean().default(false),
});

// ── Kasumi config ───────────────────────────────────────────────────────

export const kasumiConfigSchema = z.object({
  enabled: z.boolean().default(false),
  lkm_autoload: z.boolean().default(false),
  lkm_dir: z.string().default("/data/adb/kasumi/lkm"),
  lkm_kmi_override: z.string().default(""),
  mirror_path: z.string().default("/data/adb/kasumi/mirror"),
  enable_kernel_debug: z.boolean().default(false),
  enable_stealth: z.boolean().default(false),
  enable_hidexattr: z.boolean().default(false),
  enable_selinux_fix: z.boolean().default(false),
  enable_mount_hide: z.boolean().default(false),
  enable_maps_spoof: z.boolean().default(false),
  enable_statfs_spoof: z.boolean().default(false),
  mount_hide: kasumiMountHideConfigSchema.optional(),
  statfs_spoof: kasumiStatfsSpoofConfigSchema.optional(),
  hide_uids: z.array(z.number().int().nonnegative()).default([]),
  uname_mode: kasumiUnameModeSchema.default("scoped"),
  uname: kasumiUnameConfigSchema.optional(),
  cmdline_value: z.string().default(""),
  kstat_rules: z.array(kasumiKstatRuleConfigSchema).default([]),
  maps_rules: z.array(kasumiMapsRuleConfigSchema).default([]),
});

export type KasumiConfigPayload = z.infer<typeof kasumiConfigSchema>;

// ── App config (api-config-get / api-config-patch response) ─────────────

export const customBindMountSchema = z.object({
  source: z.string(),
  target: z.string(),
});

export const appConfigSchema = z.object({
  moduledir: z.string().default("/data/adb/modules"),
  mountsource: z.string().default("KSU"),
  overlay_mode: overlayModeSchema.default("ext4"),
  disable_umount: z.boolean().default(false),
  default_mode: mountModeSchema.default("overlay"),
  daemon_startup_mode: daemonStartupModeSchema.default("on-demand"),
  custom_mounts: z.array(customBindMountSchema).default([]),
  rules: z.record(z.string(), moduleRulesSchema).default({}),
  kasumi: kasumiConfigSchema.optional(),
});

export type AppConfigPayload = z.infer<typeof appConfigSchema>;

// ── Kasumi status (kasumi-status response) ──────────────────────────────

export const kasumiRuntimeInnerSchema = z.object({
  snapshot: z.record(z.string(), z.unknown()).default({}),
  kasumi_modules: z.array(z.string()).default([]),
});

export const kasumiStatusSchema = z.object({
  status: z.string().default("disabled"),
  available: z.boolean().default(false),
  kernel_supported: z.boolean().default(false),
  protocol_version: z.number().int().nullable().default(null),
  feature_bits: z.number().int().nullable().default(null),
  feature_names: z.array(z.string()).default([]),
  hooks: z.array(z.string()).default([]),
  rule_count: z.number().int().nonnegative().default(0),
  user_hide_rule_count: z.number().int().nonnegative().default(0),
  mirror_path: z.string().default(""),
  lkm: kasumiLkmStatusSchema.optional(),
  config: kasumiConfigSchema.optional(),
  runtime: kasumiRuntimeInnerSchema.optional(),
});

export type KasumiStatusPayload = z.infer<typeof kasumiStatusSchema>;

// ── Runtime state (status command response) ────────────────────────────

export const runtimeModeStatsSchema = z.object({
  overlayfs: z.number().optional(),
  magicmount: z.number().optional(),
  kasumi: z.number().optional(),
  blacklisted: z.number().optional(),
});

export const runtimeDaemonSchema = z.object({
  alive: z.boolean().optional(),
  socket_path: z.string().optional(),
  last_refresh_ts: z.number().optional(),
});

export const runtimeStateSchema = z
  .object({
    pid: z.number().optional(),
    storage_mode: z.string().optional(),
    mount_point: z.string().optional(),
    overlay_modules: z.array(z.string()).optional(),
    magic_modules: z.array(z.string()).optional(),
    kasumi_modules: z.array(z.string()).optional(),
    custom_mounts: z.array(z.string()).optional(),
    mount_error_modules: z.array(z.string()).optional(),
    mount_error_reasons: z.record(z.string(), z.string()).optional(),
    skip_mount_modules: z.array(z.string()).optional(),
    blacklisted_modules: z.array(z.string()).optional(),
    active_mounts: z.array(z.string()).optional(),
    tmpfs_xattr_supported: z.boolean().optional(),
    mode_stats: runtimeModeStatsSchema.optional(),
    daemon: runtimeDaemonSchema.optional(),
    kasumi: z.record(z.string(), z.unknown()).optional(),
  })
  .passthrough();

export type RuntimeStatePayload = z.infer<typeof runtimeStateSchema>;
