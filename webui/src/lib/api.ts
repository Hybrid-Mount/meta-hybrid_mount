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

import { AppError } from "./api/core/error";
import { PATHS } from "./constants";
import {
  ensureDaemonAwake,
  hasExecBridge,
  runDaemonCommand,
  shouldUseMock,
} from "./api/core/bridge";
import {
  getStorageUsage,
  getSystemInfo,
  getVersion,
  init,
  openLink,
  reboot,
} from "./api/services/systemService";
import {
  loadConfigFromFile,
  resetConfigFile,
  saveConfigToFile,
} from "./api/repos/configRepo";
import {
  scanModules,
  saveModules,
  saveModuleRules,
  saveAllModuleRules,
} from "./api/services/moduleService";
import { checkInstallState } from "./api/services/installCompatibility";
import type { AppAPI } from "./api/contracts";

const RealAPI = {
  checkInstallState,
  wakeDaemon: () => ensureDaemonAwake(PATHS.BINARY),
  init,
  loadConfig: loadConfigFromFile,
  saveConfig: saveConfigToFile,
  resetConfig: async () => {
    await resetConfigFile();
  },
  scanModules,
  saveModules,
  saveModuleRules,
  saveAllModuleRules,
  getStorageUsage,
  getSystemInfo,
  getVersion,
  clearMountErrors: () =>
    runDaemonCommand(
      { type: "clear-mount-errors" },
      PATHS.BINARY,
    ) as Promise<void>,
  openLink,
  reboot,
} as AppAPI;

export { AppError, hasExecBridge, runDaemonCommand };
export type { AppAPI } from "./api/contracts";
export type { DaemonCommandPayload } from "./api/core/bridge";
const mockApi = shouldUseMock
  ? ((await import("./api.mock")).MockAPI as unknown as AppAPI)
  : null;
export const API: AppAPI = mockApi ?? RealAPI;
