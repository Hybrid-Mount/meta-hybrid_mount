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
import { MockAPI } from "./api.mock";

describe("MockAPI core interactions", () => {
  it("starts in the ready install state by default", async () => {
    await expect(MockAPI.checkInstallState()).resolves.toBe("ready");
  });

  it("reports saved configs as persisted for the next boot", async () => {
    const result = await MockAPI.saveConfig({
      ...(await MockAPI.loadConfig()),
    });
    expect(result.applied).toBe(false);
    expect(result.rebootRequired).toBe(true);
  });

  it("returns the expected init payload shape", async () => {
    const initial = await MockAPI.init();
    const config = initial.config as Record<string, unknown>;
    const status = initial.status as Record<string, unknown> & {
      mode_stats: Record<string, unknown>;
    };
    expect(Object.keys(initial).sort()).toEqual([
      "config",
      "status",
      "system_info",
      "version",
    ]);
    expect(Object.keys(config).sort()).toEqual([
      "daemon_startup_mode",
      "default_mode",
      "disable_umount",
      "moduledir",
      "mountsource",
      "overlay_mode",
      "rules",
    ]);
    expect(Object.keys(status).sort()).toEqual([
      "active_mounts",
      "blacklisted_modules",
      "magic_modules",
      "mode_stats",
      "mount_error_modules",
      "mount_point",
      "overlay_modules",
      "storage_mode",
      "tmpfs_xattr_supported",
    ]);
    expect(Object.keys(status.mode_stats).sort()).toEqual([
      "blacklisted",
      "magicmount",
      "overlayfs",
    ]);
  });
});
