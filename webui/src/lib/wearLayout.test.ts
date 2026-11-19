// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it, vi } from "vitest";
import { useWearLayout } from "./wearLayout";

describe("Wear layout selection", () => {
  it("selects Wear when the device probe identifies a watch", async () => {
    const isWatch = vi.fn().mockResolvedValue(true);
    await expect(useWearLayout("", isWatch)).resolves.toBe(true);
    expect(isWatch).toHaveBeenCalledOnce();
  });

  it("selects the phone layout when the device probe finds no watch", async () => {
    await expect(useWearLayout("", async () => false)).resolves.toBe(false);
  });

  it("honors a layout override without calling the bridge", async () => {
    const isWatch = vi.fn();
    await expect(useWearLayout("?layout=wear", isWatch)).resolves.toBe(true);
    await expect(useWearLayout("?layout=phone", isWatch)).resolves.toBe(false);
    expect(isWatch).not.toHaveBeenCalled();
  });

  it("falls back when the device probe fails", async () => {
    const isWatch = vi.fn().mockRejectedValue(new Error("Bridge unavailable"));
    await expect(useWearLayout("", isWatch)).resolves.toBe(false);
  });

  it("falls back when the bridge never calls back", async () => {
    vi.useFakeTimers();
    try {
      const result = useWearLayout("", () => new Promise(() => {}));
      await vi.runAllTimersAsync();
      await expect(result).resolves.toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });
});
