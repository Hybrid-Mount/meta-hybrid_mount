// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  MODULE_SCAN_ERROR_CODE,
  STATUS_LOAD_ERROR_CODE,
  activeBackends,
  activeMountState,
  backendDisplayKeys,
  collectStatusErrors,
  groupActiveMounts,
  statusErrorDetails,
  statusErrorRows,
  statusFailureSummary,
  storageModeKey,
  uniqueActiveMounts,
} from "./statusMounts";
import type { RunState } from "./types";

const state = (timestamp: number): RunState => ({
  timestamp,
  pid: 1,
  storage_mode: "ext4",
  mount_point: "",
  overlay_modules: [],
  magic_modules: [],
  skip_mount_modules: [],
  active_mounts: [],
  overlay_active_mounts: [],
  magic_active_mounts: [],
  vfs_modules: [],
  vfs_active_mounts: [],
  vfs_provider: null,
  vfs_error: null,
  vfs_error_modules: [],
  vfs_foreign_nomount: false,
  confirmed_active_mounts: [],
  mount_error_modules: [],
  mount_error_reasons: {},
  mount_stats: {
    total_mounts: 0,
    successful_mounts: 0,
    failed_mounts: 0,
    files_mounted: 0,
    symlinks_created: 0,
    overlayfs_mounts: 0,
    ignored_entries: 0,
  },
  mode_stats: { overlayfs: 0, magicmount: 0, vfs: 0 },
  state_load: { kind: "loaded", detail: null },
  failed_stage: null,
  failure_reason: null,
  rollback_status: "committed",
  leftover_mount_targets: [],
});

describe("active mount presentation", () => {
  it("distinguishes a missing snapshot from an empty successful snapshot", () => {
    expect(activeMountState(null, [])).toBe("not-ready");
    expect(activeMountState(state(0), [])).toBe("not-ready");
    expect(activeMountState(state(1), [])).toBe("empty");
  });

  it("deduplicates the unified list before grouping by mount root", () => {
    const mounts = [
      "/system/etc/hosts",
      "/vendor/etc/audio.xml",
      "/system/etc/hosts",
      "/system/framework/services.jar",
    ];

    expect(uniqueActiveMounts(mounts)).toEqual([
      "/system/etc/hosts",
      "/system/framework/services.jar",
      "/vendor/etc/audio.xml",
    ]);
    expect(groupActiveMounts(mounts)).toEqual([
      { root: "/system", count: 2 },
      { root: "/vendor", count: 1 },
    ]);
  });
});

describe("status failure presentation", () => {
  const labels = {
    abnormal: "Needs attention",
    loadError: "Failed to load system status",
    mountFailures: (count: number) => `Detected ${count} failed mounts`,
  };

  it("hides unavailable VFS diagnostics but retains other mount failures", () => {
    const snapshot = state(1);
    snapshot.vfs_error = "VFS provider unavailable";
    expect(statusFailureSummary(snapshot, labels, false)).toBeNull();
    snapshot.mount_stats.failed_mounts = 2;
    expect(statusFailureSummary(snapshot, labels, false)).toBe(
      "Detected 2 failed mounts",
    );
    snapshot.failure_reason = "startup failed";
    expect(statusFailureSummary(snapshot, labels, false)).toBe("startup failed");
  });

  it("prioritizes explicit VFS failures and names affected modules", () => {
    const snapshot = state(1);
    snapshot.vfs_error = "VFS read-back failed";
    snapshot.vfs_error_modules = ["vfs_mod"];

    expect(statusFailureSummary(snapshot, labels)).toBe("VFS read-back failed (vfs_mod)");
  });

  it("does not hide rollback or state-load failures behind a working version", () => {
    const snapshot = state(1);
    snapshot.rollback_status = "unverified";
    expect(statusFailureSummary(snapshot, labels)).toBe(
      "Needs attention: rollback unverified",
    );

    snapshot.rollback_status = "committed";
    snapshot.state_load = { kind: "io_error", detail: "state unreadable" };
    expect(statusFailureSummary(snapshot, labels)).toBe("state unreadable");
  });
});

describe("status error banner", () => {
  it("stays empty for a healthy boot, which is what hides the banner", () => {
    expect(collectStatusErrors(state(1))).toEqual([]);
    expect(collectStatusErrors(null)).toEqual([]);
  });

  it("reports the failed stage, the reason and the left-over targets", () => {
    const snapshot: RunState = {
      ...state(1),
      failed_stage: "magic_mount",
      failure_reason: "bind mount failed",
      rollback_status: "incomplete",
      leftover_mount_targets: ["/system/framework"],
    };

    expect(collectStatusErrors(snapshot)).toEqual([
      {
        code: "status.errorBootFailure",
        detail: "bind mount failed",
        items: ["/system/framework"],
      },
    ]);
    expect(statusErrorDetails(snapshot)).toEqual({
      stage: "magic_mount",
      rollback: "incomplete",
      version: "",
    });
  });

  it("keeps a committed rollback out of the labelled rows", () => {
    expect(statusErrorDetails(state(1)).rollback).toBe("");
    expect(
      statusErrorDetails({ ...state(1), rollback_status: "unverified" }).rollback,
    ).toBe("unverified");
  });

  it("shows a foreign provider even when this build cannot use VFS", () => {
    const snapshot: RunState = { ...state(1), vfs_foreign_nomount: true };

    expect(collectStatusErrors(snapshot)).toEqual([
      { code: "status.errorVfsForeign", detail: "", items: [] },
    ]);
  });

  it("names the modules a VFS provider failure affected", () => {
    const snapshot: RunState = {
      ...state(1),
      vfs_error: "read-back mismatch",
      vfs_error_modules: ["vfs_mod"],
      vfs_provider: "3",
    };

    expect(collectStatusErrors(snapshot)).toEqual([
      {
        code: "status.errorVfsProvider",
        detail: "read-back mismatch",
        items: ["vfs_mod"],
      },
    ]);
    expect(statusErrorDetails(snapshot).version).toBe("3");
  });

  it("lists module mount failures once per module, with their reasons", () => {
    const snapshot: RunState = {
      ...state(1),
      mount_error_modules: ["b_mod", "a_mod", "b_mod"],
      mount_error_reasons: { b_mod: "marker present" },
    };

    expect(collectStatusErrors(snapshot)).toEqual([
      {
        code: "status.errorMountError",
        detail: "a_mod\nb_mod: marker present",
        items: ["a_mod", "b_mod"],
      },
    ]);
  });

  it("reports an unreadable snapshot and frontend failures side by side", () => {
    const snapshot: RunState = {
      ...state(1),
      state_load: { kind: "io_error", detail: "state unreadable" },
    };

    expect(
      collectStatusErrors(snapshot, {
        loadError: "status command failed",
        moduleScanError: "scan failed",
      }),
    ).toEqual([
      { code: STATUS_LOAD_ERROR_CODE, detail: "status command failed", items: [] },
      { code: MODULE_SCAN_ERROR_CODE, detail: "scan failed", items: [] },
      { code: "status.errorStateLoad", detail: "state unreadable", items: [] },
    ]);
  });

  it("keeps the most severe problem first when several are reported", () => {
    const snapshot: RunState = {
      ...state(1),
      failure_reason: "startup failed",
      vfs_foreign_nomount: true,
      mount_error_modules: ["a_mod"],
      mount_error_reasons: { a_mod: "marker present" },
      state_load: { kind: "corrupt", detail: "bad json" },
    };

    expect(collectStatusErrors(snapshot).map((error) => error.code)).toEqual([
      "status.errorBootFailure",
      "status.errorVfsForeign",
      "status.errorMountError",
      "status.errorStateLoad",
    ]);
  });

  it("drops blank rows so the banner never shows an empty label", () => {
    const rows = statusErrorRows(
      { stage: "plan", rollback: "", version: "" },
      { stage: "Stage", rollback: "Rollback", version: "Version" },
    );

    expect(rows).toEqual([{ label: "Stage", value: "plan" }]);
  });
});

describe("active backend presentation", () => {
  it("reports no storage backend for a VFS-only snapshot", () => {
    const vfsOnly: RunState = {
      ...state(1),
      storage_mode: "none",
      mode_stats: { overlayfs: 0, magicmount: 0, vfs: 2 },
    };

    expect(activeBackends(vfsOnly)).toEqual(["vfs"]);
    expect(backendDisplayKeys(vfsOnly)).toEqual(["config.modeVfs"]);
    expect(storageModeKey(vfsOnly)).toBeNull();
  });

  it("names tmpfs or ext4 only when that staging backend ran", () => {
    expect(storageModeKey(state(1))).toBe("config.overlayExt4");
    expect(backendDisplayKeys(state(1))).toEqual(["config.overlayExt4"]);
    expect(storageModeKey({ ...state(1), storage_mode: "tmpfs" })).toBe(
      "config.overlayTmpfs",
    );
  });

  it("lists every backend that did work in pipeline order", () => {
    const mixed: RunState = {
      ...state(1),
      storage_mode: "tmpfs",
      mode_stats: { overlayfs: 1, magicmount: 1, vfs: 1 },
    };

    expect(activeBackends(mixed)).toEqual(["overlay", "magic", "vfs"]);
    expect(backendDisplayKeys(mixed)).toEqual([
      "config.overlayTmpfs",
      "config.modeMagic",
      "config.modeVfs",
    ]);
  });

  it("reports nothing before a snapshot exists", () => {
    expect(activeBackends(null)).toEqual([]);
    expect(backendDisplayKeys(null)).toEqual([]);
  });
});
