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

import type { MountMode } from "../../types";

export function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object";
}

export function isString(value: unknown): value is string {
  return typeof value === "string";
}

export function isBoolean(value: unknown): value is boolean {
  return typeof value === "boolean";
}

export function isNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

export function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(isString);
}

export function toNonNegativeInt(value: unknown, fallback = 0): number {
  if (isNumber(value)) {
    return Math.max(0, Math.trunc(value));
  }
  if (isString(value) && /^\d+$/.test(value)) {
    return Number.parseInt(value, 10);
  }
  return fallback;
}

export function normalizeMountMode(
  value: unknown,
  fallback: MountMode = "overlay",
): MountMode {
  if (value === "magic" || value === "ignore") {
    return value;
  }
  if (value === "overlay" || value === "auto") {
    return "overlay";
  }
  return fallback;
}
