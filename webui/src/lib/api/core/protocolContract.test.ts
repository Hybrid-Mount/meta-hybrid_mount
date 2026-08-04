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
import rustProtocol from "../../../../../src/core/daemon/protocol.rs?raw";
import tsBridge from "./bridge.ts?raw";
import { DAEMON_COMMAND_TYPES } from "./protocol.generated";

function uniqueSorted(matches: IterableIterator<RegExpMatchArray>): string[] {
  return [...new Set(Array.from(matches, (match) => match[1]))].sort();
}

describe("daemon protocol contract", () => {
  it("keeps the WebUI command surface within the Rust protocol", () => {
    const webuiProtocolSource = rustProtocol.split("// ── Kasumi:")[0];
    const rustCommandTypes = uniqueSorted(
      webuiProtocolSource.matchAll(/#\[serde\(rename = "([^"]+)"\)\]/g),
    );
    const tsCommandTypes = uniqueSorted(
      tsBridge.matchAll(/\|\s*\{\s*type:\s*"([^"]+)"/g),
    );
    const generatedCommandTypes = [...DAEMON_COMMAND_TYPES].sort();

    expect(generatedCommandTypes).toEqual(rustCommandTypes);
    expect(tsCommandTypes.every((type) => rustCommandTypes.includes(type))).toBe(
      true,
    );
    expect(generatedCommandTypes.every((type) => !type.includes("kasumi"))).toBe(
      true,
    );
    expect(tsCommandTypes).not.toContain("kasumi-status");
    expect(tsCommandTypes).not.toContain("lkm-load");
  });
});
