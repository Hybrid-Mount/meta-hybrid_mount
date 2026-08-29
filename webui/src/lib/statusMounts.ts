// SPDX-License-Identifier: Apache-2.0

import type { RunState } from "./types";

export type ActiveMountState = "not-ready" | "empty" | "active";

export interface ActiveMountGroup {
  root: string;
  count: number;
}

export function uniqueActiveMounts(mounts: readonly string[]): string[] {
  return [...new Set(mounts.map((mount) => mount.trim()).filter(Boolean))].sort();
}

export function activeMountState(
  state: RunState | null | undefined,
  mounts: readonly string[],
): ActiveMountState {
  if (!state || state.timestamp <= 0) return "not-ready";
  return mounts.length > 0 ? "active" : "empty";
}

function mountRoot(mount: string): string {
  if (!mount.startsWith("/")) return mount;
  const separator = mount.indexOf("/", 1);
  return separator === -1 ? mount : mount.slice(0, separator);
}

export function groupActiveMounts(mounts: readonly string[]): ActiveMountGroup[] {
  const counts = new Map<string, number>();
  for (const mount of uniqueActiveMounts(mounts)) {
    const root = mountRoot(mount);
    counts.set(root, (counts.get(root) ?? 0) + 1);
  }

  return [...counts]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([root, count]) => ({ root, count }));
}
