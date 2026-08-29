// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { activeMountState, groupActiveMounts, uniqueActiveMounts } from "./statusMounts";
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
  mode_stats: { overlayfs: 0, magicmount: 0 },
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
