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

import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webuiRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(scriptDir, "../..");
const outputFile = path.join(webuiRoot, "src/lib/constants_gen.ts");

async function readCargoVersion() {
  const cargoToml = await readFile(path.join(repoRoot, "Cargo.toml"), "utf8");
  const version = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) {
    throw new Error("failed to read package version from Cargo.toml");
  }
  return version;
}

async function readUpgradeEpoch() {
  const source = await readFile(
    path.join(repoRoot, "xtask/src/build_meta_shared.rs"),
    "utf8",
  );
  const epoch = source.match(
    /^pub const UPGRADE_EPOCH: &str = "([^"]+)";/m,
  )?.[1];
  if (!epoch) {
    throw new Error("failed to read UPGRADE_EPOCH from build metadata");
  }
  return epoch;
}

function parseBooleanEnv(name, defaultValue) {
  const value = process.env[name]?.trim().toLowerCase();
  if (!value) return defaultValue;
  if (["1", "true", "yes", "on"].includes(value)) return true;
  if (["0", "false", "no", "off"].includes(value)) return false;
  throw new Error(`invalid boolean value for ${name}: ${process.env[name]}`);
}

function renderConstants({ version, isRelease, upgradeEpoch }) {
  return `export const APP_VERSION = "${version}";
export const IS_RELEASE = ${isRelease};
export const UPGRADE_EPOCH = "${upgradeEpoch}";
export const RUST_PATHS = {
  CONFIG: "/data/adb/hybrid-mount/config.toml",
  DAEMON_STATE: "/data/adb/hybrid-mount/run/daemon_state.json",
  BINARY: "/data/adb/modules/hybrid_mount/hybrid-mount",
  MODULE_DIR: "/data/adb/modules/hybrid_mount",
  MODULE_UPDATE_DIR: "/data/adb/modules_update/hybrid_mount",
} as const;
`;
}

const version =
  process.env.HYBRID_MOUNT_WEBUI_VERSION || (await readCargoVersion());
const isRelease = parseBooleanEnv("HYBRID_MOUNT_WEBUI_RELEASE", false);
const upgradeEpoch = await readUpgradeEpoch();
await mkdir(path.dirname(outputFile), { recursive: true });
await writeFile(
  outputFile,
  renderConstants({ version, isRelease, upgradeEpoch }),
  "utf8",
);
console.log(`Generated ${path.relative(webuiRoot, outputFile)}.`);
