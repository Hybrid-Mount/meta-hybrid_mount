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
import { AppError } from "./error";
import {
  buildSseUrl,
  parseDaemonJsonOutput,
  readModuleProp,
  resolveShouldUseMock,
  shouldUseMock,
} from "./bridge";

describe("buildSseUrl", () => {
  it("builds SSE URLs with encoded bearer tokens", () => {
    expect(
      buildSseUrl({
        base_url: "http://127.0.0.1:42321",
        token: "token with + and /",
      }),
    ).toBe("http://127.0.0.1:42321/events?token=token%20with%20%2B%20and%20%2F");
  });
});

describe("parseDaemonJsonOutput", () => {
  it("enables the mock API in test mode", () => {
    expect(shouldUseMock).toBe(true);
  });

  it("uses mock mode by default during vite dev", () => {
    expect(resolveShouldUseMock({ MODE: "development", DEV: true })).toBe(true);
  });

  it("allows disabling mock mode explicitly", () => {
    expect(
      resolveShouldUseMock({
        MODE: "development",
        DEV: true,
        VITE_USE_MOCK: "false",
      }),
    ).toBe(false);
  });

  it("allows enabling mock mode explicitly outside dev and test", () => {
    expect(
      resolveShouldUseMock({
        MODE: "production",
        DEV: false,
        VITE_USE_MOCK: "true",
      }),
    ).toBe(true);
  });

  it("parses valid JSON payloads", () => {
    expect(parseDaemonJsonOutput('{"storage_mode":"tmpfs"}')).toEqual({
      storage_mode: "tmpfs",
    });
  });

  it("parses daemon config payloads", () => {
    expect(
      parseDaemonJsonOutput(
        '{"moduledir":"/data/adb/modules","overlay_mode":"tmpfs"}',
      ),
    ).toEqual({
      moduledir: "/data/adb/modules",
      overlay_mode: "tmpfs",
    });
  });

  it("throws structured CLI error payloads", () => {
    expect(() =>
      parseDaemonJsonOutput(
        '{"type":"error","error":"Failed to connect to daemon socket"}',
      ),
    ).toThrow(AppError);
  });

  it("throws daemon response error payloads", () => {
    expect(() =>
      parseDaemonJsonOutput('{"ok":false,"error":"daemon request failed"}'),
    ).toThrow("daemon request failed");
  });

  it("rejects module.prop reads outside KSU environment in tests", async () => {
    await expect(readModuleProp("/tmp/module")).rejects.toThrow(
      "No KSU environment",
    );
  });
});
