import { describe, expect, it } from "vitest";
import { AppError } from "./error";
import {
  parseDaemonJsonOutput,
  readModuleProp,
  resolveShouldUseMock,
  shouldUseMock,
} from "./bridge";

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
