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

import { describe, expect, it, vi } from "vitest";
import type { AppAPI } from "./api/contracts";
import { runStartupGate } from "./appStartup";

function startupApi(
  state: "ready" | "clean-reinstall-required",
): Pick<AppAPI, "checkInstallState" | "init"> {
  return {
    checkInstallState: vi.fn().mockResolvedValue(state),
    init: vi.fn().mockResolvedValue({}),
  };
}

describe("app startup gate", () => {
  it("never initializes the daemon-backed API when a clean reinstall is required", async () => {
    const api = startupApi("clean-reinstall-required");

    await expect(runStartupGate(api)).resolves.toEqual({
      state: "clean-reinstall-required",
    });
    expect(api.init).not.toHaveBeenCalled();
  });

  it("initializes exactly once after a ready compatibility check", async () => {
    const api = startupApi("ready");

    await expect(runStartupGate(api)).resolves.toMatchObject({
      state: "ready",
    });
    expect(api.init).toHaveBeenCalledTimes(1);
  });

  it("does not initialize after the caller has been disposed", async () => {
    const api = startupApi("ready");

    await expect(runStartupGate(api, () => false)).resolves.toEqual({
      state: "cancelled",
    });
    expect(api.init).not.toHaveBeenCalled();
  });
});
