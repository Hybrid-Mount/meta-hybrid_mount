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
        rules: {
          default_mode: "overlay",
          paths: {},
        },
        is_blacklisted: false,
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
        rules: {
          default_mode: "overlay",
          paths: {},
        },
        is_blacklisted: false,
      },
    ]);
  });

  it("rejects empty metadata fields", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "invalid_mod",
        name: "",
        version: "2.0.0",
        author: " ",
        mode: "overlay",
        is_mounted: true,
        enabled: true,
        rules: {
          default_mode: "overlay",
          paths: {},
        },
        is_blacklisted: false,
      },
    ]);

    await expect(scanModules()).rejects.toThrow();
  });

  it("rejects the old payload shape without metadata", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "broken_mod",
        mode: "overlay",
        is_mounted: true,
        enabled: true,
        rules: {
          default_mode: "overlay",
          paths: {},
        },
        is_blacklisted: false,
      },
    ]);

    await expect(scanModules()).rejects.toThrow();
  });

  it("keeps the explicit blacklist state from the runtime payload", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "broken_mod",
        name: "Broken Module",
        version: "1.0.0",
        author: "Developer",
        description: "Broken module",
        mode: "overlay",
        is_mounted: false,
        enabled: false,
        is_blacklisted: true,
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
      is_blacklisted: true,
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
