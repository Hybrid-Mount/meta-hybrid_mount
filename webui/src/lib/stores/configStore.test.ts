// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", () => ({
  API: {
    loadConfig: vi.fn(),
    saveConfig: vi.fn(),
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

  it("keeps failed config loads retryable and refuses to save defaults", async () => {
    vi.mocked(API.loadConfig).mockRejectedValue(new Error("unreadable config"));
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    configStore.setConfig({
      ...DEFAULT_CONFIG,
      moduledir: "/stale/modules",
      default_mode: "magic",
    });

    await configStore.loadConfig();
    const saved = await configStore.saveConfig();

    expect(configStore.config).toEqual(DEFAULT_CONFIG);
    expect(configStore.hasLoaded).toBe(false);
    expect(API.loadConfig).toHaveBeenCalledTimes(1);
    expect(API.saveConfig).not.toHaveBeenCalled();
    expect(saved).toBe(false);
    expect(uiStore.showToast).toHaveBeenCalledWith(
      "Failed to load config; using defaults",
    );
  });

  it("retries loading after a temporary failure", async () => {
    vi.mocked(API.loadConfig)
      .mockRejectedValueOnce(new Error("bridge unavailable"))
      .mockResolvedValueOnce({ ...DEFAULT_CONFIG, default_mode: "magic" });
    vi.spyOn(console, "error").mockImplementation(() => undefined);

    await configStore.loadConfig();
    await configStore.ensureConfigLoaded();

    expect(API.loadConfig).toHaveBeenCalledTimes(2);
    expect(configStore.config.default_mode).toBe("magic");
    expect(configStore.hasLoaded).toBe(true);
  });
});
