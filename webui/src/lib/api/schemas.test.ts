/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { describe, expect, it } from "vitest";
import { initPayloadSchema } from "./schemas";

function idleInitPayload() {
  return {
    status: {
      timestamp: 0,
      pid: 0,
      storage_mode: "tmpfs",
      mount_point: "/data/adb/hybrid-mount",
      mounted: false,
      overlay_modules: [],
      magic_modules: [],
      custom_mounts: [],
      skip_mount_modules: [],
      blacklisted_modules: [],
      active_mounts: [],
      tmpfs_xattr_supported: false,
      mount_stats: {
        total_mounts: 0,
        successful_mounts: 0,
        failed_mounts: 0,
        tmpfs_created: 0,
        files_mounted: 0,
        dirs_mounted: 0,
        symlinks_created: 0,
        overlayfs_mounts: 0,
        ignored_entries: 0,
      },
      mode_stats: {
        overlayfs: 0,
        magicmount: 0,
        blacklisted: 0,
      },
      daemon: {
        alive: true,
        socket_path: "/data/adb/hybrid-mount/run/daemon.sock",
        last_refresh_ts: 1,
      },
    },
    config: {
      moduledir: "/data/adb/modules",
      mountsource: "KSU",
      overlay_mode: "tmpfs",
      disable_umount: false,
      default_mode: "overlay",
      custom_mounts: [],
      rules: {},
    },
    version: { version: "4.2.0" },
    system_info: {
      kernel: "6.12.69-android16",
      selinux: "Enforcing",
      mount_base: "/data/adb/hybrid-mount",
      active_mounts: [],
      tmpfs_xattr_supported: false,
      supported_overlay_modes: ["tmpfs", "ext4"],
    },
  };
}

describe("initPayloadSchema", () => {
  it("accepts the daemon idle state used when persisted state is unavailable", () => {
    const payload = initPayloadSchema.parse(idleInitPayload());

    expect(payload.status.mounted).toBe(false);
    expect(payload.status.storage_mode).toBe("tmpfs");
    expect(payload.system_info.mount_base).toBe("/data/adb/hybrid-mount");
  });

  it("rejects the legacy empty state that exposed raw validation errors", () => {
    const payload = idleInitPayload();
    payload.status.storage_mode = "";
    payload.status.mount_point = "";
    payload.system_info.mount_base = "";

    const result = initPayloadSchema.safeParse(payload);
    if (result.success) {
      throw new Error("legacy empty runtime state unexpectedly passed validation");
    }

    const issuePaths = result.error.issues.map((issue) => issue.path.join("."));
    expect(issuePaths).toEqual(
      expect.arrayContaining([
        "status.storage_mode",
        "status.mount_point",
        "system_info.mount_base",
      ]),
    );
  });
});
