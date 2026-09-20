// SPDX-License-Identifier: Apache-2.0

import type { RunState } from "./types";

export type ActiveMountState = "not-ready" | "empty" | "active";

export interface ActiveMountGroup {
  root: string;
  count: number;
}

/// Backends that can do work in one boot, in pipeline order.
export type BackendId = "overlay" | "magic" | "vfs";

/// True only when a real Tmpfs/Ext4 staging backend was created this boot.
export function hasOverlayStorage(state: RunState | null | undefined): boolean {
  return state?.storage_mode === "tmpfs" || state?.storage_mode === "ext4";
}

/**
 * Backends that actually did work in this snapshot, in pipeline order.
 *
 * OverlayFS counts only when a staging backend was really created, so a VFS-only or
 * Magic-only boot never advertises Tmpfs/Ext4.
 */
export function activeBackends(state: RunState | null | undefined): BackendId[] {
  if (!state) return [];

  const backends: BackendId[] = [];
  if (hasOverlayStorage(state)) backends.push("overlay");
  if ((state.mode_stats?.magicmount ?? 0) > 0) backends.push("magic");
  if ((state.mode_stats?.vfs ?? 0) > 0) backends.push("vfs");
  return backends;
}

/**
 * i18n keys naming the backends that actually ran, for the backend summary.
 *
 * A VFS-only boot maps to `config.modeVfs` instead of the storage mode, which never
 * started on that device.
 */
export function backendDisplayKeys(state: RunState | null | undefined): string[] {
  return activeBackends(state).map((backend) => {
    if (backend === "overlay") {
      return state?.storage_mode === "tmpfs"
        ? "config.overlayTmpfs"
        : "config.overlayExt4";
    }
    return backend === "magic" ? "config.modeMagic" : "config.modeVfs";
  });
}

/** i18n key for the overlay staging backend, or null when none was created. */
export function storageModeKey(state: RunState | null | undefined): string | null {
  if (state?.storage_mode === "tmpfs") return "config.overlayTmpfs";
  if (state?.storage_mode === "ext4") return "config.overlayExt4";
  return null;
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
