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

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../core/bridge", () => ({
  runDaemonCommand: vi.fn(),
}));

import { runDaemonCommand } from "../core/bridge";
import { saveModuleRules, scanModules } from "./moduleService";

const mockRunDaemonCommand = vi.mocked(runDaemonCommand);

describe("scanModules", () => {
  beforeEach(() => {
    mockRunDaemonCommand.mockReset();
  });

  it("uses module metadata from the daemon payload", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "hybrid_mount",
        name: "Hybrid Mount",
        version: "v3.5.6-1648",
        author: "Hybrid Mount Developers",
        description: "Waiting for daemon...",
        mode: "overlay",
        is_mounted: true,
        enabled: true,
        source_path: "/data/adb/modules/hybrid_mount",
        rules: {
          default_mode: "overlay",
          paths: {},
        },
      },
    ]);

    await expect(scanModules()).resolves.toEqual([
      {
        id: "hybrid_mount",
        name: "Hybrid Mount",
        version: "v3.5.6-1648",
        author: "Hybrid Mount Developers",
        description: "Waiting for daemon...",
        mode: "overlay",
        is_mounted: true,
        enabled: true,
        source_path: "/data/adb/modules/hybrid_mount",
        rules: {
          default_mode: "overlay",
          paths: {},
        },
      },
    ]);
  });

  it("falls back when metadata fields are missing or empty", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "fallback_mod",
        name: "",
        version: "2.0.0",
        author: " ",
        mode: "overlay",
        is_mounted: true,
        enabled: true,
        source_path: "/modules/fallback_mod",
        rules: {
          default_mode: "overlay",
          paths: {},
        },
      },
    ]);

    const modules = await scanModules();
    expect(modules[0]).toMatchObject({
      id: "fallback_mod",
      name: "fallback_mod",
      version: "2.0.0",
      author: "unknown",
      description: "No description",
    });
  });

  it("falls back when the daemon payload uses the old shape without metadata", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "broken_mod",
        mode: "overlay",
        is_mounted: true,
        enabled: true,
        source_path: "/modules/broken_mod",
        rules: {
          default_mode: "overlay",
          paths: {},
        },
      },
    ]);

    const modules = await scanModules();
    expect(modules[0]).toMatchObject({
      id: "broken_mod",
      name: "broken_mod",
      version: "unknown",
      author: "unknown",
      description: "No description",
    });
  });

  it("keeps mount error details from the runtime payload", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "broken_mod",
        mode: "overlay",
        is_mounted: false,
        enabled: false,
        source_path: "/modules/broken_mod",
        mount_error: "stage=execute; error=overlay failed",
        rules: {
          default_mode: "overlay",
          paths: {},
        },
      },
    ]);

    const modules = await scanModules();
    expect(modules[0]).toMatchObject({
      id: "broken_mod",
      is_mounted: false,
      enabled: false,
      mount_error: "stage=execute; error=overlay failed",
    });
  });
});

describe("saveModuleRules", () => {
  beforeEach(() => {
    mockRunDaemonCommand.mockReset();
  });

  it("sends a rules-only module apply payload", async () => {
    mockRunDaemonCommand.mockResolvedValue(undefined);

    await saveModuleRules("alpha", {
      default_mode: "magic",
      paths: { "/system/bin/app_process": "overlay" },
    });

    expect(mockRunDaemonCommand).toHaveBeenCalledWith(
      {
        type: "api-modules-apply",
        modules: [
          {
            id: "alpha",
            rules: {
              default_mode: "magic",
              paths: { "/system/bin/app_process": "overlay" },
            },
          },
        ],
      },
      expect.any(String),
    );
  });
});
