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

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(scriptDir, "../..");
const protocolPath = resolve(rootDir, "src/core/daemon/protocol.rs");
const outputPath = resolve(
  rootDir,
  "webui/src/lib/api/core/protocol.generated.ts",
);
const checkMode = process.argv.includes("--check");

function extractCommandTypes(source) {
  const webuiProtocolSource = source.split("// ── Kasumi:")[0];
  return [
    ...new Set(
      Array.from(
        webuiProtocolSource.matchAll(/#\[serde\(rename = "([^"]+)"\)\]/g),
        (match) => match[1],
      ),
    ),
  ].sort();
}

function render(commandTypes) {
  const values = commandTypes.map((type) => `  "${type}",`).join("\n");
  return `/*
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

// Generated from the non-Kasumi commands in src/core/daemon/protocol.rs.
// Do not edit by hand.

export const DAEMON_COMMAND_TYPES = [
${values}
] as const;

export type DaemonCommandType = (typeof DAEMON_COMMAND_TYPES)[number];
`;
}

const protocolSource = await readFile(protocolPath, "utf8");
const generated = render(extractCommandTypes(protocolSource));

if (checkMode) {
  const current = await readFile(outputPath, "utf8");
  // Git may check the file out with CRLF on Windows; compare normalized.
  if (current.replace(/\r\n/g, "\n") !== generated) {
    console.error(
      "protocol.generated.ts is stale; run pnpm generate:daemon-protocol",
    );
    process.exit(1);
  }
} else {
  await writeFile(outputPath, generated);
}
