import { describe, expect, it } from "vitest";
import { MockAPI } from "./api.mock";

describe("MockAPI Kasumi controls", () => {
  it("keeps runtime rule clearing independent from config toggles", async () => {
    await MockAPI.setKasumiStealth(true);
    await MockAPI.setKasumiHidexattr(false);
    await MockAPI.setKasumiDebug(true);

    const beforeClear = await MockAPI.getKasumiStatus();
    expect(beforeClear.rule_count).toBeGreaterThan(0);
    expect(beforeClear.config.enable_stealth).toBe(true);
    expect(beforeClear.config.enable_hidexattr).toBe(false);
    expect(beforeClear.config.enable_kernel_debug).toBe(true);

    await MockAPI.clearKasumiRules();

    const afterClear = await MockAPI.getKasumiStatus();
    expect(afterClear.rule_count).toBe(0);
    expect(afterClear.config.enable_stealth).toBe(true);
    expect(afterClear.config.enable_hidexattr).toBe(false);
    expect(afterClear.config.enable_kernel_debug).toBe(true);

    await MockAPI.setKasumiStealth(false);
    await MockAPI.setKasumiHidexattr(true);
    await MockAPI.setKasumiDebug(false);

    const afterToggle = await MockAPI.getKasumiStatus();
    expect(afterToggle.config.enable_stealth).toBe(false);
    expect(afterToggle.config.enable_hidexattr).toBe(true);
    expect(afterToggle.config.enable_kernel_debug).toBe(false);
  });
});
