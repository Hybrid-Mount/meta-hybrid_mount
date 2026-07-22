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

describe("MockAPI Kasumi controls", () => {
  it("keeps runtime rule clearing independent from config toggles", async () => {
    await MockAPI.setKasumiStealth(true);
    await MockAPI.setKasumiOverlayXattrHide(false);
    await MockAPI.setKasumiDebug(true);

    const beforeClear = await MockAPI.getKasumiStatus();
    expect(beforeClear.rule_count).toBeGreaterThan(0);
    expect(beforeClear.config.enable_stealth).toBe(true);
    expect(beforeClear.config.enable_overlay_xattr_hide).toBe(false);
    expect(beforeClear.config.enable_kernel_debug).toBe(true);

    await MockAPI.clearKasumiRules();

    const afterClear = await MockAPI.getKasumiStatus();
    expect(afterClear.rule_count).toBe(0);
    expect(afterClear.config.enable_stealth).toBe(true);
    expect(afterClear.config.enable_overlay_xattr_hide).toBe(false);
    expect(afterClear.config.enable_kernel_debug).toBe(true);

    await MockAPI.setKasumiStealth(false);
    await MockAPI.setKasumiOverlayXattrHide(true);
    await MockAPI.setKasumiDebug(false);

    const afterToggle = await MockAPI.getKasumiStatus();
    expect(afterToggle.config.enable_stealth).toBe(false);
    expect(afterToggle.config.enable_overlay_xattr_hide).toBe(true);
    expect(afterToggle.config.enable_kernel_debug).toBe(false);
  });
});
