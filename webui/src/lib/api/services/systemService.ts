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

import { PATHS } from "../../constants";
import type { StorageStatus, SystemInfo } from "../../types";
import type { InitPayload } from "../contracts";
import { runDaemonCommand } from "../core/bridge";
import {
  initPayloadSchema,
  systemInfoSchema,
  versionSchema,
  runtimeStateSchema,
} from "../schemas";
import { buildModeStats, buildMountedCount } from "../codec/runtimeCodec";

export async function init(): Promise<InitPayload> {
  const raw = await runDaemonCommand({ type: "init" }, PATHS.BINARY);
  const payload = initPayloadSchema.parse(raw);
  return {
    status: payload.status,
    config: payload.config,
    version: payload.version.version,
    system_info: payload.system_info,
  } as InitPayload;
}

export async function getStorageUsage(): Promise<StorageStatus> {
  const state = runtimeStateSchema.parse(
    await runDaemonCommand({ type: "status" }, PATHS.BINARY),
  );
  return {
    type: state.storage_mode,
    mounted: state.mounted,
    modeStats: buildModeStats(state),
    mountedCount: buildMountedCount(state),
  };
}

export async function getSystemInfo(): Promise<SystemInfo> {
  const payload = systemInfoSchema.parse(
    await runDaemonCommand({ type: "api-system-info" }, PATHS.BINARY),
  );
  return {
    kernel: payload.kernel,
    selinux: payload.selinux,
    mountBase: payload.mount_base,
    activeMounts: payload.active_mounts,
    tmpfs_xattr_supported: payload.tmpfs_xattr_supported,
    supported_overlay_modes: payload.supported_overlay_modes,
  };
}

export async function getVersion(): Promise<string> {
  const payload = versionSchema.parse(
    await runDaemonCommand({ type: "api-version" }, PATHS.BINARY),
  );
  return payload.version;
}

export async function reboot(): Promise<void> {
  await runDaemonCommand({ type: "api-reboot" }, PATHS.BINARY);
}

export async function openLink(url: string): Promise<void> {
  await runDaemonCommand({ type: "api-open-url", url }, PATHS.BINARY);
}
