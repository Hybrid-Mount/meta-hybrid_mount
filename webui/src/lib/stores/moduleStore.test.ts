// SPDX-License-Identifier: Apache-2.0

import { computed } from "vue";
import { expect, it, vi } from "vitest";
import { API } from "../api";
import { moduleStore } from "./moduleStore";

vi.mock("../api", () => ({ API: { scanModules: vi.fn() } }));
vi.mock("./uiStore", () => ({ uiStore: { showToast: vi.fn() } }));

it("invalidates a failed module scan and retries on the next visit", async () => {
  const loaded = computed(() => moduleStore.hasLoaded);
  const scan = vi.mocked(API.scanModules);
  scan
    .mockResolvedValueOnce([])
    .mockRejectedValueOnce(new Error("offline"))
    .mockResolvedValueOnce([]);

  expect(loaded.value).toBe(false);
  await moduleStore.ensureModulesLoaded();
  expect(loaded.value).toBe(true);
  await moduleStore.loadModules();
  expect(loaded.value).toBe(false);
  expect(moduleStore.loadError).toBe("offline");
  await moduleStore.ensureModulesLoaded();
  expect(loaded.value).toBe(true);
  expect(moduleStore.loadError).toBeNull();
  expect(scan).toHaveBeenCalledTimes(3);
});
