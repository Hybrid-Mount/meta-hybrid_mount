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

import { PATHS } from "../../constants";
import type { Module, ModuleRules } from "../../types";
import { runDaemonCommand } from "../core/bridge";
import {
  moduleRuntimeEntrySchema,
  type ModuleRuntimeEntryRaw,
} from "../schemas";
import { normalizeMountMode } from "../core/guards";

interface ModuleMetadata {
  name: string;
  version: string;
  author: string;
  description: string;
}

type ModuleApplyRulesPayload = {
  default_mode: ModuleRules["default_mode"];
  paths: ModuleRules["paths"];
};

type ModuleRulesApplyEntry = { id: string; rules: ModuleApplyRulesPayload };

function defaultModuleMetadata(moduleId: string): ModuleMetadata {
  return {
    name: moduleId,
    version: "unknown",
    author: "unknown",
    description: "No description",
  };
}

function normalizeMetadataField(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function extractMetadata(entry: ModuleRuntimeEntryRaw): ModuleMetadata {
  const defaults = defaultModuleMetadata(entry.id);
  return {
    name: normalizeMetadataField(entry.name) ?? defaults.name,
    version: normalizeMetadataField(entry.version) ?? defaults.version,
    author: normalizeMetadataField(entry.author) ?? defaults.author,
    description:
      normalizeMetadataField(entry.description) ?? defaults.description,
  };
}

function toModule(
  entry: ModuleRuntimeEntryRaw,
  metadata: ModuleMetadata,
): Module {
  const rules = entry.rules ?? { default_mode: "overlay" as const, paths: {} };
  return {
    id: entry.id,
    name: metadata.name,
    version: metadata.version,
    author: metadata.author,
    description: metadata.description,
    mode: normalizeMountMode(entry.mode),
    is_mounted: entry.is_mounted,
    enabled: entry.enabled,
    source_path: entry.source_path,
    rules: {
      default_mode: normalizeMountMode(rules.default_mode),
      paths: rules.paths ?? {},
    },
    mount_error: entry.mount_error?.trim() || undefined,
    suggest_ignore: entry.suggest_ignore,
  };
}

function moduleRulesPayload(rules: ModuleRules): ModuleApplyRulesPayload {
  return {
    default_mode: normalizeMountMode(rules.default_mode),
    paths: rules.paths ?? {},
  };
}

function moduleRulesApplyEntry(
  moduleId: string,
  rules: ModuleRules,
): ModuleRulesApplyEntry {
  return {
    id: moduleId,
    rules: moduleRulesPayload(rules),
  };
}

export async function scanModules(path?: string): Promise<Module[]> {
  const payload = await runDaemonCommand(
    {
      type: "api-modules-list",
      path: path?.trim() || null,
    },
    PATHS.BINARY,
  );
  if (!Array.isArray(payload)) {
    throw new Error("modules payload is invalid");
  }

  const entries = payload.map((item) => moduleRuntimeEntrySchema.parse(item));
  return entries.map((entry) => toModule(entry, extractMetadata(entry)));
}

export async function saveModuleRules(
  moduleId: string,
  rules: ModuleRules,
): Promise<void> {
  await runDaemonCommand(
    {
      type: "api-modules-apply",
      modules: [moduleRulesApplyEntry(moduleId, rules)],
    },
    PATHS.BINARY,
  );
}
