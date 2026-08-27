// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { cloneAppConfig } from "./config";
import type { AppConfig } from "./types";

describe("cloneAppConfig", () => {
  it("deep-clones editable module rules without structuredClone", () => {
    const source: AppConfig = {
      moduledir: "/data/adb/modules",
      mountsource: "KSU",
      overlay_mode: "ext4",
      tmpfs_xattr_supported: false,
      disable_umount: false,
      default_mode: "overlay",
      rules: {
        demo: {
          default_mode: "magic",
          paths: { "system/etc/hosts": "overlay" },
        },
      },
    };

    const cloned = cloneAppConfig(source);
    cloned.rules.demo.paths["system/etc/hosts"] = "ignore";

    expect(cloned).not.toBe(source);
    expect(cloned.rules).not.toBe(source.rules);
    expect(cloned.rules.demo).not.toBe(source.rules.demo);
    expect(cloned.rules.demo.paths).not.toBe(source.rules.demo.paths);
    expect(source.rules.demo.paths["system/etc/hosts"]).toBe("overlay");
  });
});
