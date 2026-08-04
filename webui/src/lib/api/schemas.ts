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

export const mountModeSchema = z.enum(["overlay", "magic", "ignore"]);
export const overlayModeSchema = z.enum(["tmpfs", "ext4"]);

export const moduleRulesSchema = z
  .object({
    default_mode: mountModeSchema,
    paths: z.record(z.string(), mountModeSchema),
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

export const customBindMountSchema = z
  .object({ source: z.string(), target: z.string() })
  .strict();

// Strip dormant backend-only fields so WebUI sends back only the currently
// supported configuration surface.
export const appConfigSchema = z
  .object({
    moduledir: z.string(),
    mountsource: z.string(),
    overlay_mode: overlayModeSchema,
    disable_umount: z.boolean(),
    default_mode: mountModeSchema,
    custom_mounts: z.array(customBindMountSchema),
    rules: z.record(z.string(), moduleRulesSchema),
  })
  .strip();

export type AppConfigPayload = z.infer<typeof appConfigSchema>;

export const runtimeModeStatsSchema = z
  .object({
    overlayfs: z.number().int().nonnegative(),
    magicmount: z.number().int().nonnegative(),
    blacklisted: z.number().int().nonnegative(),
  })
  .strip();

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
    custom_mounts: z.array(z.string()),
    skip_mount_modules: z.array(z.string()),
    blacklisted_modules: z.array(z.string()),
    active_mounts: z.array(z.string()),
    tmpfs_xattr_supported: z.boolean(),
    mount_stats: runtimeMountStatsSchema,
    mode_stats: runtimeModeStatsSchema,
    daemon: runtimeDaemonSchema,
  })
  .strip();

export type RuntimeStatePayload = z.infer<typeof runtimeStateSchema>;

export const initPayloadSchema = z
  .object({
    status: runtimeStateSchema,
    config: appConfigSchema,
    version: versionSchema,
    system_info: systemInfoSchema,
  })
  .strip();

export type InitPayloadRaw = z.infer<typeof initPayloadSchema>;
