/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 */

import { afterEach, describe, expect, it, vi } from "vitest";
import { API } from "../api";
import { DEFAULT_CONFIG } from "../constants";
import type { AppConfig } from "../types";
import { configStore } from "./configStore";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("config store writes", () => {
  afterEach(() => vi.restoreAllMocks());

  it("ignores stale patch responses and tracks all active saves", async () => {
    const first = deferred<AppConfig>();
    const second = deferred<AppConfig>();
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

    second.resolve({ ...DEFAULT_CONFIG, mountsource: "second" });
    await secondSave;
    expect(configStore.config.mountsource).toBe("second");
    expect(configStore.saving).toBe(true);

    first.resolve({ ...DEFAULT_CONFIG, mountsource: "first" });
    await firstSave;
    expect(configStore.config.mountsource).toBe("second");
    expect(configStore.saving).toBe(false);
  });
});
