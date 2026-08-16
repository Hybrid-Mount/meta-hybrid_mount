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

import { afterEach, describe, expect, it, vi } from "vitest";
import { API } from "../api";
import { DEFAULT_CONFIG } from "../constants";
import type { ConfigPatchResult } from "../api/contracts";
import type { AppConfig } from "../types";
import { configStore } from "./configStore";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function patchResult(config: AppConfig): ConfigPatchResult {
  return { config, applied: false, rebootRequired: true };
}

describe("config store writes", () => {
  afterEach(() => vi.restoreAllMocks());

  it("ignores stale patch responses and tracks all active saves", async () => {
    const first = deferred<ConfigPatchResult>();
    const second = deferred<ConfigPatchResult>();
    vi.spyOn(API, "patchConfig")
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);

    configStore.config = { ...DEFAULT_CONFIG, mountsource: "initial" };
    configStore.setField("mountsource", "first");
    const firstSave = configStore.patchConfig(
      { mountsource: "first" },
      { showSuccess: false, showError: false },
    );
    configStore.setField("mountsource", "second");
    const secondSave = configStore.patchConfig(
      { mountsource: "second" },
      { showSuccess: false, showError: false },
    );

    second.resolve(patchResult({ ...DEFAULT_CONFIG, mountsource: "second" }));
    await secondSave;
    expect(configStore.config.mountsource).toBe("second");
    expect(configStore.saving).toBe(true);

    first.resolve(patchResult({ ...DEFAULT_CONFIG, mountsource: "first" }));
    await firstSave;
    expect(configStore.config.mountsource).toBe("second");
    expect(configStore.saving).toBe(false);
  });
});
