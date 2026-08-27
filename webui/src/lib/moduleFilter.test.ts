// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { matchesModuleFilter } from "./moduleFilter";

describe("matchesModuleFilter", () => {
  it("hides ignored modules from the default active filter", () => {
    expect(matchesModuleFilter({ mode: "overlay" }, "active")).toBe(true);
    expect(matchesModuleFilter({ mode: "magic" }, "active")).toBe(true);
    expect(matchesModuleFilter({ mode: "ignore" }, "active")).toBe(false);
  });

  it("still exposes ignored modules through explicit filters", () => {
    expect(matchesModuleFilter({ mode: "ignore" }, "all")).toBe(true);
    expect(matchesModuleFilter({ mode: "ignore" }, "ignore")).toBe(true);
  });
});
