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
import { parseInstallState, previewInstallState } from "./installCompatibility";

describe("install compatibility state", () => {
  it("treats module properties without an upgrade state as ready", () => {
    expect(parseInstallState(["upgradeEpoch=1\n"])).toBe("ready");
  });

  it("requires a clean reinstall for versions from before the epoch mechanism", () => {
    expect(parseInstallState(["id=hybrid_mount\n"])).toBe(
      "clean-reinstall-required",
    );
  });

  it("requires a clean reinstall for a different epoch", () => {
    expect(parseInstallState(["upgradeEpoch=0\n"])).toBe(
      "clean-reinstall-required",
    );
  });

  it("keeps a legacy active module blocked beside a current staged module", () => {
    expect(parseInstallState(["id=hybrid_mount\n", "upgradeEpoch=1\n"])).toBe(
      "clean-reinstall-required",
    );
  });

  it("allows matching active and staged module epochs", () => {
    expect(parseInstallState(["upgradeEpoch=1\n", "upgradeEpoch=1\n"])).toBe(
      "ready",
    );
  });

  it("recognizes the sticky clean reinstall state with CRLF input", () => {
    expect(
      parseInstallState([
        "upgradeEpoch=1\r\nupgradeState=clean-reinstall-required\r\n",
      ]),
    ).toBe("clean-reinstall-required");
  });

  it("fails closed for an unknown upgrade state", () => {
    expect(() =>
      parseInstallState(["upgradeEpoch=1\nupgradeState=unknown\n"]),
    ).toThrow("unsupported upgrade state");
  });

  it("fails closed when no installed module properties are readable", () => {
    expect(() => parseInstallState([null, null])).toThrow(
      "module properties are unavailable",
    );
  });

  it("supports an explicit development preview query", () => {
    expect(previewInstallState("?reinstall-required=1")).toBe(
      "clean-reinstall-required",
    );
    expect(previewInstallState("?reinstall-required=true")).toBe(
      "clean-reinstall-required",
    );
    expect(previewInstallState("")).toBe("ready");
  });
});
