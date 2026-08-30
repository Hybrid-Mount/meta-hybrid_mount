// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", () => ({
  API: {
    loadConfig: vi.fn(),
  },
}));

vi.mock("./uiStore", () => ({
  uiStore: {
    showToast: vi.fn(),
  },
}));

import { API } from "../api";
import { DEFAULT_CONFIG } from "../constants";
import { configStore } from "./configStore";
import { uiStore } from "./uiStore";

describe("configStore", () => {
  afterEach(() => vi.restoreAllMocks());

  it("replaces stale state with defaults when config loading fails", async () => {
    vi.mocked(API.loadConfig).mockRejectedValueOnce(new Error("unreadable config"));
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    configStore.setConfig({
      ...DEFAULT_CONFIG,
      moduledir: "/stale/modules",
      default_mode: "magic",
    });

    await configStore.loadConfig();
    await configStore.ensureConfigLoaded();

    expect(configStore.config).toEqual(DEFAULT_CONFIG);
    expect(configStore.hasLoaded).toBe(true);
    expect(API.loadConfig).toHaveBeenCalledTimes(1);
    expect(uiStore.showToast).toHaveBeenCalledWith(
      "Failed to load config; using defaults",
    );
  });
});
