// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  createApi,
  createConfigPayload,
  normalizeConfigPayload,
  normalizeModule,
  normalizeInstallState,
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

  it("saves global settings without overwriting independently edited module rules", () => {
    const config: AppConfig = {
      moduledir: "/data/adb/modules",
      overlay_mode: "ext4",
      tmpfs_xattr_supported: false,
      disable_umount: false,
      default_mode: "overlay",
      rules: {
        inherited: { default_mode: null, paths: {} },
      },
    };

    expect(createConfigPayload(config)).toEqual({
      moduledir: config.moduledir,
      overlay_mode: config.overlay_mode,
      disable_umount: config.disable_umount,
      default_mode: config.default_mode,
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

  it("merges and deduplicates active mounts from every backend", () => {
    const status = normalizeStatus({
      timestamp: 1,
      active_mounts: ["/system", "/system/etc/hosts"],
      overlay_active_mounts: ["/system"],
      magic_active_mounts: ["/system/etc/hosts", "/vendor/etc/audio.xml"],
      vfs_active_mounts: ["/vendor/etc/audio.xml", "/product/etc/build.prop"],
    });

    expect(status.active_mounts).toEqual([
      "/product/etc/build.prop",
      "/system",
      "/system/etc/hosts",
      "/vendor/etc/audio.xml",
    ]);
    expect(status.overlay_active_mounts).toEqual(["/system"]);
    expect(status.magic_active_mounts).toEqual([
      "/system/etc/hosts",
      "/vendor/etc/audio.xml",
    ]);
    expect(status.vfs_active_mounts).toEqual([
      "/product/etc/build.prop",
      "/vendor/etc/audio.xml",
    ]);
  });

  it("counts VFS injection points as active mounts for a VFS-only snapshot", () => {
    const status = normalizeStatus({
      timestamp: 1,
      storage_mode: "none",
      active_mounts: [],
      overlay_active_mounts: [],
      magic_active_mounts: [],
      vfs_active_mounts: ["/system/etc/hosts"],
    });

    expect(status.active_mounts).toEqual(["/system/etc/hosts"]);
  });

  it("does not read a missing overlay storage mode as ext4", () => {
    expect(normalizeStatus({ timestamp: 1 }).storage_mode).toBe("none");
    expect(normalizeStatus({ timestamp: 1, storage_mode: "" }).storage_mode).toBe("none");
    expect(normalizeStatus({ timestamp: 1, storage_mode: "ext4" }).storage_mode).toBe(
      "ext4",
    );
  });

  it("defaults backend-specific mount lists for older snapshots", () => {
    const status = normalizeStatus({ timestamp: 1, active_mounts: ["/system"] });

    expect(status.active_mounts).toEqual(["/system"]);
    expect(status.overlay_active_mounts).toEqual([]);
    expect(status.magic_active_mounts).toEqual([]);
    expect(status.vfs_active_mounts).toEqual([]);
  });

  it("defaults VFS failure diagnostics for older snapshots", () => {
    const status = normalizeStatus({ timestamp: 1 });

    expect(status.vfs_error).toBeNull();
    expect(status.vfs_error_modules).toEqual([]);
  });

  it("preserves startup and rollback diagnostics", () => {
    const status = normalizeStatus({
      timestamp: 1,
      state_load: { kind: "loaded", detail: "snapshot detail" },
      failed_stage: "mount_execution",
      failure_reason: "overlay failed",
      rollback_status: "incomplete",
      leftover_mount_targets: ["/system"],
      confirmed_active_mounts: ["/vendor"],
      vfs_foreign_nomount: true,
    });

    expect(status).toMatchObject({
      state_load: { kind: "loaded", detail: "snapshot detail" },
      failed_stage: "mount_execution",
      failure_reason: "overlay failed",
      rollback_status: "incomplete",
      leftover_mount_targets: ["/system"],
      confirmed_active_mounts: ["/vendor"],
      vfs_foreign_nomount: true,
    });
  });
});

describe("VFS capability contract", () => {
  it("requires an affirmative runtime probe, including for older backends", () => {
    for (const value of [undefined, null, false, "true", "false"]) {
      expect(normalizeInstallState({ vfs_supported: value }).vfs_supported).toBe(false);
    }
    expect(normalizeInstallState({ vfs_supported: true }).vfs_supported).toBe(true);
  });
});

describe("installation capability status", () => {
  it("keeps unverified Nuke distinct from an observed failure", () => {
    for (const value of [undefined, null, "true", "false", 1]) {
      const state = normalizeInstallState({
        tmpfs_supported: value,
        nuke_supported: value,
      });
      expect(state.tmpfs_supported).toBe(false);
      expect(state.nuke_supported).toBeNull();
    }
    for (const value of [true, false]) {
      const state = normalizeInstallState({
        tmpfs_supported: value,
        nuke_supported: value,
      });
      expect(state.tmpfs_supported).toBe(value);
      expect(state.nuke_supported).toBe(value);
    }
  });
});
