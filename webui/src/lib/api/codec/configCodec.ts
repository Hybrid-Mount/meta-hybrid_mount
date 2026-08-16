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

import type { AppConfig } from "../../types";
import type { ConfigPatchResult } from "../contracts";
import { appConfigSchema } from "../schemas";

export function normalizeConfig(value: unknown): AppConfig {
  return appConfigSchema.parse(value) as AppConfig;
}

export function normalizeConfigPatchResult(value: unknown): ConfigPatchResult {
  if (!value || typeof value !== "object") {
    throw new Error("daemon config patch result is not an object");
  }
  const payload = value as Record<string, unknown>;
  if (
    typeof payload.config !== "object" ||
    payload.config === null ||
    typeof payload.applied !== "boolean" ||
    typeof payload.reboot_required !== "boolean"
  ) {
    throw new Error(
      "daemon config patch result is missing config, applied, or reboot_required",
    );
  }
  return {
    config: normalizeConfig(payload.config),
    applied: payload.applied,
    rebootRequired: payload.reboot_required,
  };
}
