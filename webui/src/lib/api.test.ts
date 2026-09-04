// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  createApi,
  createConfigPayload,
  normalizeConfigPayload,
  normalizeModule,
  normalizeStatus,
} from "./api";
import type { AppConfig } from "./types";

describe("WebUI configuration contract", () => {
  it("rejects production API calls when the manager bridge is unavailable", async () => {
    const api = createApi(false, false);

    await expect(api.loadConfig()).rejects.toThrow(
      "KernelSU/APatch WebUI bridge is unavailable",
    );
  });

  it("marks full editor saves as rule replacements", () => {
    const config: AppConfig = {
      moduledir: "/data/adb/modules",
      mountsource: "KSU",
      overlay_mode: "ext4",
      tmpfs_xattr_supported: false,
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

  it("recognizes blacklisted modules and legacy blacklist markers", () => {
    expect(normalizeModule({ id: "blocked", blacklisted: true }).blacklisted).toBe(true);
    const legacyModule = normalizeModule({
      id: "legacy-blocked",
      mount_error: "blacklisted",
    });
    expect(legacyModule.blacklisted).toBe(true);
    expect(legacyModule.mount_error).toBeNull();
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

  it("hides unsupported tmpfs configurations behind the ext4 fallback", () => {
    const unsupported = normalizeConfigPayload({
      overlay_mode: "tmpfs",
      tmpfs_xattr_supported: false,
    });
    const supported = normalizeConfigPayload({
      overlay_mode: "tmpfs",
      tmpfs_xattr_supported: true,
    });

    expect(unsupported.overlay_mode).toBe("ext4");
    expect(unsupported.tmpfs_xattr_supported).toBe(false);
    expect(supported.overlay_mode).toBe("tmpfs");
    expect(supported.tmpfs_xattr_supported).toBe(true);
  });

  it("does not accept ignore as the global default", () => {
    const config = normalizeConfigPayload({ default_mode: "ignore" });

    expect(config.default_mode).toBe("overlay");
  });

  it("merges and deduplicates active mounts from both backends", () => {
    const status = normalizeStatus({
      timestamp: 1,
      active_mounts: ["/system", "/system/etc/hosts"],
      overlay_active_mounts: ["/system"],
      magic_active_mounts: ["/system/etc/hosts", "/vendor/etc/audio.xml"],
    });

    expect(status.active_mounts).toEqual([
      "/system",
      "/system/etc/hosts",
      "/vendor/etc/audio.xml",
    ]);
    expect(status.overlay_active_mounts).toEqual(["/system"]);
    expect(status.magic_active_mounts).toEqual([
      "/system/etc/hosts",
      "/vendor/etc/audio.xml",
    ]);
  });

  it("defaults backend-specific mount lists for older snapshots", () => {
    const status = normalizeStatus({ timestamp: 1, active_mounts: ["/system"] });

    expect(status.active_mounts).toEqual(["/system"]);
    expect(status.overlay_active_mounts).toEqual([]);
    expect(status.magic_active_mounts).toEqual([]);
  });
});
