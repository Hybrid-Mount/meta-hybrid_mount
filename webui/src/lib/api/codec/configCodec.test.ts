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
import { normalizeConfig } from "./configCodec";

describe("config codec defaults", () => {
  it("matches the Rust default overlay storage mode", () => {
    expect(DEFAULT_CONFIG.overlay_mode).toBe("ext4");
    expect(normalizeConfig({}).overlay_mode).toBe("ext4");
    expect(appConfigSchema.parse({}).overlay_mode).toBe("ext4");
  });

  it("uses a valid mount source schema default", () => {
    expect(appConfigSchema.parse({}).mountsource).toBe("KSU");
  });

  it("normalizes custom bind mounts from snake or camel case payloads", () => {
    expect(
      normalizeConfig({
        custom_mounts: [{ source: "/data/local/foo", target: "/system/foo" }],
      }).custom_mounts,
    ).toEqual([{ source: "/data/local/foo", target: "/system/foo" }]);

    expect(
      normalizeConfig({
        customMounts: [{ source: "/data/local/bar", target: "/system/bar" }],
      }).custom_mounts,
    ).toEqual([{ source: "/data/local/bar", target: "/system/bar" }]);
  });
});
