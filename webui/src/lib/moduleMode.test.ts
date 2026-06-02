import { describe, expect, it } from "vitest";
import { normalizeMountMode } from "./api/core/guards";

describe("normalizeMountMode", () => {
  it("returns magic when mode is magic", () => {
    expect(normalizeMountMode("magic")).toBe("magic");
  });

  it("returns kasumi when mode is kasumi", () => {
    expect(normalizeMountMode("kasumi")).toBe("kasumi");
  });

  it("returns ignore when mode is ignore", () => {
    expect(normalizeMountMode("ignore")).toBe("ignore");
  });

  it("falls back to overlay for unknown mode", () => {
    expect(normalizeMountMode("unknown")).toBe("overlay");
  });
});
