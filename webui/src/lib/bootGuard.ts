// SPDX-License-Identifier: Apache-2.0

import type { BootGuardReport, BootGuardState } from "./types";

/** Verdict of a marker that does not exist; the only one a tap cannot remove. */
export const GUARD_ABSENT = "absent";

const VERDICTS = ["absent", "own", "foreign", "stale", "unattributed"];

function normalizeGuard(raw: unknown): BootGuardState | null {
  if (!raw || typeof raw !== "object") return null;
  const record = raw as Record<string, unknown>;
  const name = typeof record.name === "string" ? record.name : "";
  const path = typeof record.path === "string" ? record.path : "";
  const verdict = typeof record.verdict === "string" ? record.verdict : "";
  if (!name || !path || !VERDICTS.includes(verdict)) return null;
  return {
    name,
    path,
    verdict,
    contents: typeof record.contents === "string" ? record.contents : null,
  };
}

/**
 * Validates the payload of `hybrid-mount vfs guard --json`.
 *
 * Whether a marker gates the current build is a decision only the binary can make, so an
 * unknown verdict is dropped instead of being rendered as a clearable problem.
 */
export function normalizeGuardReport(payload: unknown): BootGuardReport {
  if (!payload || typeof payload !== "object") {
    throw new Error("vfs guard returned an unexpected payload");
  }
  const record = payload as Record<string, unknown>;
  const guards = Array.isArray(record.guards)
    ? record.guards
        .map(normalizeGuard)
        .filter((guard): guard is BootGuardState => guard !== null)
    : [];
  const cleared = Array.isArray(record.cleared)
    ? record.cleared.filter((path): path is string => typeof path === "string")
    : [];
  return { guards, cleared };
}

/** Markers a tap on the banner action would actually remove. */
export function clearableGuards(report: BootGuardReport | null): BootGuardState[] {
  if (!report) return [];
  return report.guards.filter((guard) => guard.verdict !== GUARD_ABSENT);
}
