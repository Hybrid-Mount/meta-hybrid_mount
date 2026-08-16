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
import { DEFAULT_CONFIG } from "../../constants";
import { appConfigSchema } from "../schemas";
import {
  normalizeConfig,
  normalizeConfigPatchResult,
} from "./configCodec";

describe("config codec", () => {
  it("accepts the complete canonical config", () => {
    expect(DEFAULT_CONFIG.overlay_mode).toBe("ext4");
    expect(normalizeConfig(DEFAULT_CONFIG)).toEqual(DEFAULT_CONFIG);
  });

  it("accepts and normalizes a daemon config patch result", () => {
    const result = normalizeConfigPatchResult({
      saved: true,
      applied: false,
      reboot_required: true,
      config: DEFAULT_CONFIG,
    });

    expect(result.config).toEqual(DEFAULT_CONFIG);
    expect(result.applied).toBe(false);
    expect(result.rebootRequired).toBe(true);
  });

  it("rejects an incomplete daemon config patch result", () => {
    expect(() =>
      normalizeConfigPatchResult({ saved: true, config: DEFAULT_CONFIG }),
    ).toThrow();
  });

  it("rejects missing config fields", () => {
    expect(() => appConfigSchema.parse({})).toThrow();
  });

  it("accepts snake-case custom mounts and rejects the old camel-case field", () => {
    expect(
      normalizeConfig({
        ...DEFAULT_CONFIG,
        custom_mounts: [{ source: "/data/local/foo", target: "/system/foo" }],
      }).custom_mounts,
    ).toEqual([{ source: "/data/local/foo", target: "/system/foo" }]);

    expect(() =>
      normalizeConfig({
        ...DEFAULT_CONFIG,
        custom_mounts: undefined,
        customMounts: [{ source: "/data/local/bar", target: "/system/bar" }],
      }),
    ).toThrow();
  });
});
