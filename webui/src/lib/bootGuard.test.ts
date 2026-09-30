// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";

import { clearableGuards, normalizeGuardReport } from "./bootGuard";

describe("normalizeGuardReport", () => {
  it("keeps well-formed guards and the cleared paths", () => {
    const report = normalizeGuardReport({
      guards: [
        {
          name: "rules",
          path: "/data/adb/hybrid-mount/vfs_boot_guard",
          verdict: "own",
          contents: "version=6.2.3-rc.1\nsource=boot\n",
        },
      ],
      cleared: ["/data/adb/hybrid-mount/vfs_lkm_boot_guard"],
    });

    expect(report.guards).toEqual([
      {
        name: "rules",
        path: "/data/adb/hybrid-mount/vfs_boot_guard",
        verdict: "own",
        contents: "version=6.2.3-rc.1\nsource=boot\n",
      },
    ]);
    expect(report.cleared).toEqual(["/data/adb/hybrid-mount/vfs_lkm_boot_guard"]);
  });

  it("reports no marker as null contents", () => {
    const report = normalizeGuardReport({
      guards: [
        {
          name: "lkm",
          path: "/data/adb/hybrid-mount/vfs_lkm_boot_guard",
          verdict: "absent",
          contents: null,
        },
      ],
    });

    expect(report.guards[0].contents).toBeNull();
    expect(report.cleared).toEqual([]);
  });

  it("drops a guard whose verdict the binary never returns", () => {
    const report = normalizeGuardReport({
      guards: [
        { name: "rules", path: "/x", verdict: "present" },
        { name: "rules", path: "/y", verdict: "foreign" },
      ],
    });

    expect(report.guards.map((guard) => guard.path)).toEqual(["/y"]);
  });

  it("drops a guard without a name or path", () => {
    const report = normalizeGuardReport({
      guards: [
        { name: "", path: "/x", verdict: "own" },
        { name: "rules", verdict: "own" },
      ],
    });

    expect(report.guards).toEqual([]);
  });

  it("rejects a payload that is not an object", () => {
    expect(() => normalizeGuardReport("absent")).toThrow(/unexpected payload/);
  });
});

describe("clearableGuards", () => {
  it("ignores the absent marker and keeps the rest", () => {
    const guards = clearableGuards(
      normalizeGuardReport({
        guards: [
          { name: "rules", path: "/rules", verdict: "absent", contents: null },
          { name: "lkm", path: "/lkm", verdict: "unattributed", contents: "lkm=/x" },
        ],
      }),
    );

    expect(guards.map((guard) => guard.path)).toEqual(["/lkm"]);
  });

  it("offers nothing before the first report arrives", () => {
    expect(clearableGuards(null)).toEqual([]);
  });
});
