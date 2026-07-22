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

type ModuleApplyRulesPayload = {
  default_mode: ModuleRules["default_mode"];
  paths: ModuleRules["paths"];
};

type ModuleRulesApplyEntry = { id: string; rules: ModuleApplyRulesPayload };

function toModule(entry: ModuleRuntimeEntryRaw): Module {
  return {
    id: entry.id,
    name: entry.name,
    version: entry.version,
    author: entry.author,
    description: entry.description,
    mode: entry.mode,
    is_mounted: entry.is_mounted,
    enabled: entry.enabled,
    rules: entry.rules,
    is_blacklisted: entry.is_blacklisted,
  };
}

function moduleRulesPayload(rules: ModuleRules): ModuleApplyRulesPayload {
  return {
    default_mode: rules.default_mode,
    paths: rules.paths,
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

export async function scanModules(): Promise<Module[]> {
  const payload = await runDaemonCommand(
    { type: "api-modules-list" },
    PATHS.BINARY,
  );
  if (!Array.isArray(payload)) {
    throw new Error("modules payload is invalid");
  }

  const entries = payload.map((item) => moduleRuntimeEntrySchema.parse(item));
  return entries.map(toModule);
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
