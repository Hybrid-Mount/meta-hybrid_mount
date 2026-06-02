import type { MountMode } from "../../types";
import { ENABLE_KASUMI } from "../../constants_gen";

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
  if (value === "kasumi") {
    return ENABLE_KASUMI ? "kasumi" : fallback;
  }
  if (value === "magic" || value === "ignore") {
    return value;
  }
  if (value === "overlay" || value === "auto") {
    return "overlay";
  }
  return fallback;
}
