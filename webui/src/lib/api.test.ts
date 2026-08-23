// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { createConfigPayload, normalizeConfigPayload, normalizeModule } from "./api";
import type { AppConfig } from "./types";

describe("WebUI configuration contract", () => {
  it("marks full editor saves as rule replacements", () => {
    const config: AppConfig = {
      moduledir: "/data/adb/modules",
      mountsource: "KSU",
      overlay_mode: "ext4",
      disable_umount: false,
      default_mode: "overlay",
      rules: {
        inherited: { default_mode: null, paths: {} },
      },
    };

    expect(createConfigPayload(config)).toMatchObject({
      replace_rules: true,
      rules: {
        inherited: { default_mode: null, paths: {} },
      },
    });
  });

  it("preserves a module's inherited default mode", () => {
    const module = normalizeModule({
      id: "demo",
      mode: "magic",
      rules: {
        default_mode: null,
        paths: {
          "system/etc/hosts": "overlay",
          invalid: "unsupported",
        },
      },
    });

    expect(module.rules.default_mode).toBeNull();
    expect(module.rules.paths).toEqual({ "system/etc/hosts": "overlay" });
  });

  it("normalizes explicit and inherited config rules without freezing defaults", () => {
    const config = normalizeConfigPayload({
      default_mode: "magic",
      rules: {
        inherited: { default_mode: null, paths: {} },
        explicit: { default_mode: "ignore", paths: {} },
      },
    });

    expect(config.default_mode).toBe("magic");
    expect(config.rules.inherited.default_mode).toBeNull();
    expect(config.rules.explicit.default_mode).toBe("ignore");
  });
});
