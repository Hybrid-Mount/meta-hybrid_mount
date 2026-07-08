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

import { DEFAULT_CONFIG } from "../../constants";
import { ENABLE_KASUMI } from "../../constants_gen";
import type { AppConfig, ModuleRules, OverlayMode } from "../../types";
import { normalizeMountMode as normalizeMountModeBase } from "../core/guards";
import { kasumiConfigSchema } from "../schemas";

export function normalizeMountMode(
  value: unknown,
  fallback: Parameters<typeof normalizeMountModeBase>[1] = "overlay",
): ReturnType<typeof normalizeMountModeBase> {
  return normalizeMountModeBase(value, fallback);
}

export function normalizeOverlayMode(value: unknown): OverlayMode {
  return value === "tmpfs" ? "tmpfs" : "ext4";
}

function normalizeKasumiConfig(value: unknown): AppConfig["kasumi"] {
  const parsed = kasumiConfigSchema.safeParse(value);
  return parsed.success
    ? (parsed.data as unknown as AppConfig["kasumi"])
    : (DEFAULT_CONFIG.kasumi as AppConfig["kasumi"]);
}

export function normalizeConfig(value: unknown): AppConfig {
  const next = (value && typeof value === "object" ? value : {}) as Record<
    string,
    unknown
  >;
  const defaultMode = normalizeMountMode(
    next.default_mode,
    DEFAULT_CONFIG.default_mode,
  );
  const rulesSource = (
    next.rules && typeof next.rules === "object" ? next.rules : {}
  ) as Record<string, unknown>;
  const rules: Record<string, ModuleRules> = {};

  for (const [moduleId, ruleValue] of Object.entries(rulesSource)) {
    if (!ruleValue || typeof ruleValue !== "object") continue;
    const r = ruleValue as Record<string, unknown>;
    rules[moduleId] = {
      default_mode: normalizeMountMode(r.default_mode, defaultMode),
      paths: normalizePathsMap(r.paths),
    };
  }

  const normalized = {
    moduledir:
      typeof next.moduledir === "string"
        ? next.moduledir
        : DEFAULT_CONFIG.moduledir,
    mountsource:
      typeof next.mountsource === "string"
        ? next.mountsource
        : DEFAULT_CONFIG.mountsource,
    overlay_mode: normalizeOverlayMode(next.overlay_mode),
    disable_umount:
      typeof next.disable_umount === "boolean"
        ? next.disable_umount
        : DEFAULT_CONFIG.disable_umount,
    default_mode: defaultMode,
    daemon_startup_mode:
      next.daemon_startup_mode === "persistent" ? "persistent" : "on-demand",
    custom_mounts: normalizeCustomMounts(
      next.custom_mounts ?? next.customMounts,
    ),
    rules,
  };

  return {
    ...normalized,
    ...(ENABLE_KASUMI ? { kasumi: normalizeKasumiConfig(next.kasumi) } : {}),
  } as AppConfig;
}

function normalizeCustomMounts(value: unknown): AppConfig["custom_mounts"] {
  if (!Array.isArray(value)) return [];

  return value
    .map((entry) => {
      if (!entry || typeof entry !== "object") return null;
      const item = entry as Record<string, unknown>;
      if (typeof item.source !== "string" || typeof item.target !== "string") {
        return null;
      }
      return {
        source: item.source,
        target: item.target,
      };
    })
    .filter((entry): entry is AppConfig["custom_mounts"][number] =>
      Boolean(entry),
    );
}

function normalizePathsMap(value: unknown): Record<string, string> {
  if (!value || typeof value !== "object") return {};
  const result: Record<string, string> = {};
  for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
    if (typeof entry === "string") {
      result[key] = normalizeMountMode(entry);
    }
  }
  return result;
}
