import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../core/bridge", () => ({
  runDaemonCommand: vi.fn(),
  readModuleProp: vi.fn(),
}));

import { readModuleProp, runDaemonCommand } from "../core/bridge";
import { scanModules } from "./moduleService";

const mockRunDaemonCommand = vi.mocked(runDaemonCommand);
const mockReadModuleProp = vi.mocked(readModuleProp);

describe("scanModules", () => {
  beforeEach(() => {
    mockRunDaemonCommand.mockReset();
    mockReadModuleProp.mockReset();
  });

  it("parses metadata from the real module.prop template shape", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "hybrid_mount",
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
    mockReadModuleProp.mockResolvedValue(`id=hybrid_mount
name=Hybrid Mount
version=v3.5.6-1648
versionCode=305006
author=Hybrid Mount Developers
description=Waiting for daemon...
updateJson=https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/update.json
metamodule=1
webuiIcon=launcher.png
`);

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

  it("ignores comments and blank lines", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "example",
        mode: "magic",
        is_mounted: false,
        enabled: true,
        source_path: "/modules/example",
        rules: {
          default_mode: "magic",
          paths: {},
        },
      },
    ]);
    mockReadModuleProp.mockResolvedValue(`# comment

name = Example Module
invalid-line
author = Alice
`);

    const modules = await scanModules();
    expect(modules[0]).toMatchObject({
      id: "example",
      name: "Example Module",
      version: "unknown",
      author: "Alice",
      description: "No description",
    });
  });

  it("falls back when metadata fields are missing or empty", async () => {
    mockRunDaemonCommand.mockResolvedValue([
      {
        id: "fallback_mod",
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
    mockReadModuleProp.mockResolvedValue("name=\nversion=2.0.0\n");

    const modules = await scanModules();
    expect(modules[0]).toMatchObject({
      id: "fallback_mod",
      name: "fallback_mod",
      version: "2.0.0",
      author: "unknown",
      description: "No description",
    });
  });

  it("falls back when reading module.prop fails", async () => {
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
    mockReadModuleProp.mockRejectedValue(new Error("ENOENT"));

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
    mockReadModuleProp.mockRejectedValue(new Error("ENOENT"));

    const modules = await scanModules();
    expect(modules[0]).toMatchObject({
      id: "broken_mod",
      is_mounted: false,
      enabled: false,
      mount_error: "stage=execute; error=overlay failed",
    });
  });
});
