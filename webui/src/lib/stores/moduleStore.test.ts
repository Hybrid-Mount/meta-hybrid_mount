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
import type { ModuleRules } from "../types";
import { uiStore } from "./uiStore";
import { moduleStore } from "./moduleStore";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const magicRules: ModuleRules = { default_mode: "magic", paths: {} };
const overlayRules: ModuleRules = { default_mode: "overlay", paths: {} };

describe("module store writes", () => {
  afterEach(() => vi.restoreAllMocks());

  it("serializes saves for the same module and tracks queued work", async () => {
    const first = deferred<void>();
    const second = deferred<void>();
    const save = vi
      .spyOn(API, "saveModuleRules")
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);
    vi.spyOn(uiStore, "showToast").mockImplementation(() => undefined);

    const firstSave = moduleStore.saveModuleRules("alpha", magicRules);
    const secondSave = moduleStore.saveModuleRules("alpha", overlayRules);
    await Promise.resolve();

    expect(save).toHaveBeenCalledTimes(1);
    expect(moduleStore.saving).toBe(true);

    first.resolve();
    await firstSave;
    await Promise.resolve();
    expect(save).toHaveBeenCalledTimes(2);
    expect(moduleStore.saving).toBe(true);

    second.resolve();
    await secondSave;
    expect(moduleStore.saving).toBe(false);
    expect(
      save.mock.calls.map(([id, rules]) => [id, rules.default_mode]),
    ).toEqual([
      ["alpha", "magic"],
      ["alpha", "overlay"],
    ]);
  });

  it("lets a newer save recover from an older failed request", async () => {
    const first = deferred<void>();
    const second = deferred<void>();
    vi.spyOn(API, "saveModuleRules")
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);
    const toast = vi
      .spyOn(uiStore, "showToast")
      .mockImplementation(() => undefined);

    const firstSave = moduleStore.saveModuleRules("beta", magicRules);
    const secondSave = moduleStore.saveModuleRules("beta", overlayRules);
    await Promise.resolve();

    first.reject(new Error("stale failure"));
    await expect(firstSave).resolves.toBe(true);
    await Promise.resolve();
    second.resolve();
    await expect(secondSave).resolves.toBe(true);

    expect(toast).toHaveBeenCalledTimes(1);
    expect(toast).toHaveBeenCalledWith(expect.any(String), "success");
    expect(moduleStore.saving).toBe(false);
  });
});
