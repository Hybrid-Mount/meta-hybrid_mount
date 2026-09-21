// SPDX-License-Identifier: Apache-2.0

import type { RunState } from "./types";

export type ActiveMountState = "not-ready" | "empty" | "active";

export interface ActiveMountGroup {
  root: string;
  count: number;
}

export interface StatusFailureLabels {
  abnormal: string;
  loadError: string;
  mountFailures: (count: number) => string;
}

/** Returns the same failure priority for every status skin. */
export function statusFailureSummary(
  state: RunState | null | undefined,
  labels: StatusFailureLabels,
  vfsSupported = true,
): string | null {
  if (!state) return null;
  if (state.failure_reason) return state.failure_reason;
  if (vfsSupported && state.vfs_error) {
    const modules = state.vfs_error_modules.filter(Boolean);
    return modules.length > 0
      ? `${state.vfs_error} (${modules.join(", ")})`
      : state.vfs_error;
  }
  if (state.failed_stage) return `${labels.abnormal}: ${state.failed_stage}`;
  if (state.mount_stats.failed_mounts > 0) {
    return labels.mountFailures(state.mount_stats.failed_mounts);
  }
  if (state.rollback_status === "incomplete" || state.rollback_status === "unverified") {
    return `${labels.abnormal}: rollback ${state.rollback_status}`;
  }
  if (state.state_load.kind === "corrupt" || state.state_load.kind === "io_error") {
    return state.state_load.detail || labels.loadError;
  }
  return null;
}

/**
 * One problem the boot snapshot reports, in the order it should be shown.
 *
 * `code` selects the localized sentence; `detail` carries the raw value that belongs to that
 * sentence (a stage name, a kernel or tool message, one reason per affected module). The banner
 * therefore never has to render a key, and every skin says the same thing.
 */
export interface StatusError {
  code: string;
  detail: string;
  /** Affected module ids or left-over mount targets, shown as chips. */
  items: string[];
}

/** The problem the frontend itself hit, which no snapshot can report. */
export const STATUS_LOAD_ERROR_CODE = "status.errorLoad";
export const MODULE_SCAN_ERROR_CODE = "status.errorModuleScan";

const BOOT_FAILURE_CODE = "status.errorBootFailure";
const VFS_FOREIGN_CODE = "status.errorVfsForeign";
const VFS_PROVIDER_CODE = "status.errorVfsProvider";
const MOUNT_ERROR_CODE = "status.errorMountError";
const STATE_LOAD_CODE = "status.errorStateLoad";

/** Extra values the banner shows as labelled rows, not as part of the sentence. */
export interface StatusErrorDetails {
  /** The pipeline stage that failed, when the snapshot names one. */
  stage: string;
  /** Rollback outcome, when it is not "committed". */
  rollback: string;
  /** The provider version the kernel answered with. */
  version: string;
}

const rollbackFailed = (status: string | null | undefined): boolean =>
  status === "incomplete" || status === "unverified";

/** Keeps the requested order, drops blanks and repeats. */
function unique(values: readonly string[]): string[] {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}

/**
 * Reasons one per affected module, sorted by module id.
 *
 * Modules without a recorded reason still appear, so the banner never hides a module that
 * failed; they just carry the module id alone.
 */
function mountErrorDetail(state: RunState): string {
  return [...unique(state.mount_error_modules)]
    .sort((left, right) => left.localeCompare(right))
    .map((id) => {
      const reason = state.mount_error_reasons[id]?.trim();
      return reason ? `${id}: ${reason}` : id;
    })
    .join("\n");
}

/**
 * Every problem worth a top-level banner, most severe first, or an empty list.
 *
 * A healthy boot returns `[]`, which is what keeps the banner off the screen. The rollout
 * status and left-over targets of an otherwise successful boot are reported too: they are the
 * difference between "the mounts worked" and "the state is trustworthy".
 */
export function collectStatusErrors(
  state: RunState | null | undefined,
  options: { loadError?: string | null; moduleScanError?: string | null } = {},
): StatusError[] {
  const errors: StatusError[] = [];
  const loadError = options.loadError?.trim();
  if (loadError) {
    errors.push({ code: STATUS_LOAD_ERROR_CODE, detail: loadError, items: [] });
  }
  const moduleScanError = options.moduleScanError?.trim();
  if (moduleScanError) {
    errors.push({
      code: MODULE_SCAN_ERROR_CODE,
      detail: moduleScanError,
      items: [],
    });
  }

  if (!state) return errors;

  if (state.failure_reason) {
    errors.push({
      code: BOOT_FAILURE_CODE,
      detail: state.failure_reason,
      items: unique(state.leftover_mount_targets),
    });
  }

  // Ungated, unlike the status summary: a foreign provider must be visible even when this
  // build cannot use VFS itself.
  if (state.vfs_foreign_nomount) {
    errors.push({ code: VFS_FOREIGN_CODE, detail: "", items: [] });
  } else if (state.vfs_error) {
    errors.push({
      code: VFS_PROVIDER_CODE,
      detail: state.vfs_error,
      items: unique(state.vfs_error_modules),
    });
  }

  if (state.mount_error_modules.some((id) => id.trim().length > 0)) {
    errors.push({
      code: MOUNT_ERROR_CODE,
      detail: mountErrorDetail(state),
      items: unique(state.mount_error_modules).sort((left, right) =>
        left.localeCompare(right),
      ),
    });
  }

  if (state.state_load.kind === "corrupt" || state.state_load.kind === "io_error") {
    errors.push({
      code: STATE_LOAD_CODE,
      detail: state.state_load.detail?.trim() ?? "",
      items: [],
    });
  }

  return errors;
}

/** Labelled values the banner shows next to the message, when the snapshot has them. */
export function statusErrorDetails(
  state: RunState | null | undefined,
  vfsSupported = true,
): StatusErrorDetails {
  return {
    stage: state?.failed_stage?.trim() ?? "",
    rollback: rollbackFailed(state?.rollback_status)
      ? (state?.rollback_status ?? "")
      : "",
    version: vfsSupported && state?.vfs_provider ? state.vfs_provider.trim() : "",
  };
}

/**
 * Labelled rows for the values that belong beside the sentence rather than inside it.
 *
 * Every skin renders these three rows in this order, so a stage name is never mistaken for a
 * kernel message.
 */
export function statusErrorRows(
  details: StatusErrorDetails,
  labels: { stage: string; rollback: string; version: string },
): { label: string; value: string }[] {
  return [
    { label: labels.stage, value: details.stage },
    { label: labels.rollback, value: details.rollback },
    { label: labels.version, value: details.version },
  ].filter((row) => row.value.length > 0);
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
export function backendDisplayKeys(
  state: RunState | null | undefined,
  vfsSupported = true,
): string[] {
  return activeBackends(state)
    .filter((backend) => backend !== "vfs" || vfsSupported)
    .map((backend) => {
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
