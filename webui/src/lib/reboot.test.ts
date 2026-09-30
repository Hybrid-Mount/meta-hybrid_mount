// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { parseLateLoad, parseRootManager } from "./reboot";

describe("KernelSU late-load parsing", () => {
  it.each([
    ["late_load: true", true],
    ["version: 123\n  late_load\t:\tfalse \r\nother: true", false],
  ])("reads one explicit boolean: %s", (output, expected) => {
    expect(parseLateLoad(output)).toBe(expected);
  });

  it.each([
    "",
    "version: 123",
    "not_late_load: false",
    "late_load:",
    "late_load: TRUE",
    "late_load: false trailing",
    "late_load = false",
    "late_load: true\nlate_load: false",
    "late_load: false\nlate_load: false",
    "late_load: false\nlate_load: invalid",
  ])("refuses missing, malformed, or ambiguous booleans: %s", (output) => {
    expect(() => parseLateLoad(output)).toThrow(/late.load/i);
  });
});

describe("root manager environment parsing", () => {
  it.each([
    ["KSU=true\nAPATCH=\n", "kernelsu"],
    ["KSU=true\nAPATCH=false\n", "kernelsu"],
    ["KSU=\nAPATCH=true\n", "apatch"],
    ["KSU=false\nAPATCH=true\n", "apatch"],
    ["KSU=\nAPATCH=\n", null],
    ["KSU=false\nAPATCH=false\n", null],
  ])("reads explicit identity without guessing: %s", (output, expected) => {
    expect(parseRootManager(output)).toBe(expected);
  });

  it.each([
    "",
    "APATCH=true\n",
    "KSU=true\nAPATCH=true\n",
    "KSU=unknown\nAPATCH=true\n",
    "KSU=\nAPATCH=true\nKSU=true\n",
  ])("refuses malformed or conflicting identity: %s", (output) => {
    expect(() => parseRootManager(output)).toThrow(/root environment/i);
  });
});
