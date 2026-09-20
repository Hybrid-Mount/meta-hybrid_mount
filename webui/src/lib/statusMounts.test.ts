// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  activeBackends,
  activeMountState,
  backendDisplayKeys,
  groupActiveMounts,
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
