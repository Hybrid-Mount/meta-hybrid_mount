// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it, vi } from "vitest";
import { useWearLayout } from "./wearLayout";

describe("Wear layout selection", () => {
  it("selects Wear for a watch with a compact viewport", async () => {
    const isWatch = vi.fn().mockResolvedValue(true);
    await expect(useWearLayout("", 192, isWatch)).resolves.toBe(true);
    expect(isWatch).toHaveBeenCalledOnce();
  });

  it("keeps a narrow phone viewport on the phone layout", async () => {
    await expect(useWearLayout("", 192, async () => false)).resolves.toBe(false);
  });

  it("skips the device probe on a phone-sized viewport", async () => {
    const isWatch = vi.fn().mockResolvedValue(true);
    await expect(useWearLayout("", 360, isWatch)).resolves.toBe(false);
    expect(isWatch).not.toHaveBeenCalled();
  });

  it("honors a layout override without calling the bridge", async () => {
    const isWatch = vi.fn();
    await expect(useWearLayout("?layout=wear", 360, isWatch)).resolves.toBe(true);
    await expect(useWearLayout("?layout=phone", 192, isWatch)).resolves.toBe(false);
    expect(isWatch).not.toHaveBeenCalled();
  });

  it("falls back when the device probe fails", async () => {
    const isWatch = vi.fn().mockRejectedValue(new Error("Bridge unavailable"));
    await expect(useWearLayout("", 192, isWatch)).resolves.toBe(false);
  });

  it("falls back when the bridge never calls back", async () => {
    vi.useFakeTimers();
    try {
      const result = useWearLayout("", 192, () => new Promise(() => {}));
      await vi.runAllTimersAsync();
      await expect(result).resolves.toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });
});
