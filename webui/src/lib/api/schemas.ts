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

import { z } from "zod/v4";

export const mountModeSchema = z.enum(["overlay", "magic", "kasumi", "ignore"]);
export const overlayModeSchema = z.enum(["tmpfs", "ext4"]);
export const kasumiUnameModeSchema = z.enum(["scoped", "global"]);

export const moduleRulesSchema = z
  .object({
    default_mode: mountModeSchema,
    paths: z.record(z.string(), z.string()),
  })
  .strict();

export type ModuleRulesPayload = z.infer<typeof moduleRulesSchema>;

export const moduleRuntimeEntrySchema = z
  .object({
    id: z.string(),
    name: z.string().min(1),
    version: z.string().min(1),
    author: z.string().min(1),
    description: z.string().min(1),
    mode: mountModeSchema,
    is_mounted: z.boolean(),
    enabled: z.boolean(),
    rules: moduleRulesSchema,
    is_blacklisted: z.boolean(),
  })
  .strict();

export type ModuleRuntimeEntryRaw = z.infer<typeof moduleRuntimeEntrySchema>;

export const systemInfoSchema = z
  .object({
    kernel: z.string().min(1),
    selinux: z.string().min(1),
    mount_base: z.string().min(1),
    active_mounts: z.array(z.string()),
    tmpfs_xattr_supported: z.boolean(),
    supported_overlay_modes: z.array(overlayModeSchema),
  })
  .strict();

export type SystemInfoPayload = z.infer<typeof systemInfoSchema>;

export const versionSchema = z.object({ version: z.string().min(1) }).strict();

export const kernelUnameSchema = z
  .object({ release: z.string(), version: z.string() })
  .strict();

export const kasumiLkmStatusSchema = z
  .object({
    loaded: z.boolean(),
    module_name: z.string().nullable(),
    autoload: z.boolean(),
    kmi_override: z.string(),
    current_kmi: z.string().nullable(),
    search_dir: z.string(),
    module_file: z.string().min(1).nullable(),
    available_kmis: z.array(z.string().min(1)),
  })
  .strict();

export type KasumiLkmStatusPayload = z.infer<typeof kasumiLkmStatusSchema>;

export const kasumiUnameConfigSchema = z
  .object({
    sysname: z.string(),
    nodename: z.string(),
    release: z.string(),
    version: z.string(),
    machine: z.string(),
    domainname: z.string(),
  })
  .strict();

export type KasumiUnameConfigPayload = z.infer<typeof kasumiUnameConfigSchema>;

export const kasumiMountHideConfigSchema = z
  .object({ enabled: z.boolean(), path_pattern: z.string() })
  .strict();

export const kasumiStatfsSpoofConfigSchema = z
  .object({
    enabled: z.boolean(),
    path: z.string(),
    spoof_f_type: z.number().int().nonnegative(),
  })
  .strict();

export const kasumiMapsRuleConfigSchema = z
  .object({
    target_ino: z.number().int().nonnegative(),
    target_dev: z.number().int().nonnegative(),
    spoofed_ino: z.number().int().nonnegative(),
    spoofed_dev: z.number().int().nonnegative(),
    spoofed_pathname: z.string(),
  })
  .strict();

export const kasumiKstatRuleConfigSchema = z
  .object({
    target_ino: z.number().int().nonnegative(),
    target_pathname: z.string(),
    spoofed_ino: z.number().int().nonnegative(),
    spoofed_dev: z.number().int().nonnegative(),
    spoofed_nlink: z.number().int().nonnegative(),
    spoofed_size: z.number(),
    spoofed_atime_sec: z.number(),
    spoofed_atime_nsec: z.number(),
    spoofed_mtime_sec: z.number(),
    spoofed_mtime_nsec: z.number(),
    spoofed_ctime_sec: z.number(),
    spoofed_ctime_nsec: z.number(),
    spoofed_blksize: z.number().int().nonnegative(),
    spoofed_blocks: z.number().int().nonnegative(),
    is_static: z.boolean(),
  })
  .strict();

export const kasumiConfigSchema = z
  .object({
    enabled: z.boolean(),
    lkm_autoload: z.boolean(),
    lkm_dir: z.string(),
    lkm_kmi_override: z.string(),
    mirror_path: z.string(),
    enable_kernel_debug: z.boolean(),
    enable_stealth: z.boolean(),
    enable_overlay_xattr_hide: z.boolean(),
    enable_selinux_fix: z.boolean(),
    enable_mount_hide: z.boolean(),
    enable_maps_spoof: z.boolean(),
    enable_statfs_spoof: z.boolean(),
    mount_hide: kasumiMountHideConfigSchema,
    statfs_spoof: kasumiStatfsSpoofConfigSchema,
    hide_uids: z.array(z.number().int().nonnegative()),
    uname_mode: kasumiUnameModeSchema,
    uname: kasumiUnameConfigSchema,
    cmdline_value: z.string(),
    kstat_rules: z.array(kasumiKstatRuleConfigSchema),
    maps_rules: z.array(kasumiMapsRuleConfigSchema),
  })
  .strict();

export type KasumiConfigPayload = z.infer<typeof kasumiConfigSchema>;

export const customBindMountSchema = z
  .object({ source: z.string(), target: z.string() })
  .strict();

export const appConfigSchema = z
  .object({
    moduledir: z.string(),
    mountsource: z.string(),
    overlay_mode: overlayModeSchema,
    disable_umount: z.boolean(),
    default_mode: mountModeSchema,
    custom_mounts: z.array(customBindMountSchema),
    rules: z.record(z.string(), moduleRulesSchema),
    kasumi: kasumiConfigSchema,
  })
  .strict();

export type AppConfigPayload = z.infer<typeof appConfigSchema>;

export const runtimeModeStatsSchema = z
  .object({
    overlayfs: z.number().int().nonnegative(),
    magicmount: z.number().int().nonnegative(),
    kasumi: z.number().int().nonnegative(),
    blacklisted: z.number().int().nonnegative(),
  })
  .strict();

export const runtimeMountStatsSchema = z
  .object({
    total_mounts: z.number().int().nonnegative(),
    successful_mounts: z.number().int().nonnegative(),
    failed_mounts: z.number().int().nonnegative(),
    tmpfs_created: z.number().int().nonnegative(),
    files_mounted: z.number().int().nonnegative(),
    dirs_mounted: z.number().int().nonnegative(),
    symlinks_created: z.number().int().nonnegative(),
    overlayfs_mounts: z.number().int().nonnegative(),
    ignored_entries: z.number().int().nonnegative(),
  })
  .strict();

export const runtimeKasumiInfoSchema = z
  .object({
    status: z.string(),
    available: z.boolean(),
    kernel_supported: z.boolean(),
    lkm_loaded: z.boolean(),
    lkm_autoload: z.boolean(),
    lkm_kmi_override: z.string(),
    lkm_current_kmi: z.string(),
    lkm_dir: z.string(),
    protocol_version: z.number().int().nullable(),
    feature_bits: z.number().int().nullable(),
    feature_names: z.array(z.string()),
    hooks: z.array(z.string()),
    rule_count: z.number().int().nonnegative(),
    user_hide_rule_count: z.number().int().nonnegative(),
    mirror_path: z.string(),
  })
  .strict();

export const runtimeDaemonSchema = z
  .object({
    alive: z.boolean(),
    socket_path: z.string(),
    last_refresh_ts: z.number().int().nonnegative(),
  })
  .strict();

export const runtimeStateSchema = z
  .object({
    timestamp: z.number().int().nonnegative(),
    pid: z.number().int().nonnegative(),
    storage_mode: z.enum(["tmpfs", "ext4"]),
    mount_point: z.string().min(1),
    overlay_modules: z.array(z.string()),
    magic_modules: z.array(z.string()),
    kasumi_modules: z.array(z.string()),
    custom_mounts: z.array(z.string()),
    skip_mount_modules: z.array(z.string()),
    blacklisted_modules: z.array(z.string()),
    active_mounts: z.array(z.string()),
    tmpfs_xattr_supported: z.boolean(),
    mount_stats: runtimeMountStatsSchema,
    mode_stats: runtimeModeStatsSchema,
    kasumi: runtimeKasumiInfoSchema,
    daemon: runtimeDaemonSchema,
  })
  .strict();

export type RuntimeStatePayload = z.infer<typeof runtimeStateSchema>;

export const kasumiRuntimeInnerSchema = z
  .object({
    snapshot: runtimeKasumiInfoSchema,
    kasumi_modules: z.array(z.string()),
    active_mounts: z.array(z.string()),
  })
  .strict();

export const kasumiStatusSchema = z
  .object({
    status: z.string(),
    available: z.boolean(),
    kernel_supported: z.boolean(),
    protocol_version: z.number().int().nullable(),
    feature_bits: z.number().int().nullable(),
    feature_names: z.array(z.string()),
    hooks: z.array(z.string()),
    rule_count: z.number().int().nonnegative(),
    user_hide_rule_count: z.number().int().nonnegative(),
    mirror_path: z.string(),
    lkm: kasumiLkmStatusSchema,
    config: kasumiConfigSchema,
    runtime: kasumiRuntimeInnerSchema,
  })
  .strict();

export type KasumiStatusPayload = z.infer<typeof kasumiStatusSchema>;

export const initPayloadSchema = z
  .object({
    status: runtimeStateSchema,
    config: appConfigSchema,
    version: versionSchema,
    kasumi_status: kasumiStatusSchema.optional(),
    system_info: systemInfoSchema,
  })
  .strict();

export type InitPayloadRaw = z.infer<typeof initPayloadSchema>;
