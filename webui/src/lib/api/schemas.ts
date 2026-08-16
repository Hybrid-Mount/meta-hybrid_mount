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
// Complex normalisation (module metadata, etc.) stays in the
// codec layer (configCodec.ts / runtimeCodec.ts), which already handles
// missing/null values with sensible defaults.

import { z } from "zod/v4";

// ── Primitives ───────────────────────────────────────────────────────────

export const mountModeSchema = z.enum(["overlay", "magic", "ignore"]);

export const overlayModeSchema = z.enum(["tmpfs", "ext4"]);

export const daemonStartupModeSchema = z.enum(["on-demand", "persistent"]);

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

// ── Init payload ─────────────────────────────────────────────────────────

export const initPayloadSchema = z.object({
  status: z.unknown(),
  config: z.unknown(),
  version: z.unknown(),
  system_info: z.unknown(),
});

export type InitPayloadRaw = z.infer<typeof initPayloadSchema>;

// ── Runtime state (status command response) ────────────────────────────

export const runtimeModeStatsSchema = z.object({
  overlayfs: z.number().optional(),
  magicmount: z.number().optional(),
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
    mount_error_modules: z.array(z.string()).optional(),
    mount_error_reasons: z.record(z.string(), z.string()).optional(),
    skip_mount_modules: z.array(z.string()).optional(),
    blacklisted_modules: z.array(z.string()).optional(),
    active_mounts: z.array(z.string()).optional(),
    tmpfs_xattr_supported: z.boolean().optional(),
    mode_stats: runtimeModeStatsSchema.optional(),
    daemon: runtimeDaemonSchema.optional(),
  })
  .passthrough();

export type RuntimeStatePayload = z.infer<typeof runtimeStateSchema>;
