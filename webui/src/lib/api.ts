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

import { PATHS } from "./constants";
import { loadMockApi } from "./api.mock-loader";
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
  patchConfigFile,
  resetConfigFile,
} from "./api/repos/configRepo";
import { scanModules, saveModuleRules } from "./api/services/moduleService";
import type { AppAPI } from "./api/contracts";

const RealAPI = {
  wakeDaemon: () => ensureDaemonAwake(PATHS.BINARY),
  init,
  loadConfig: loadConfigFromFile,
  patchConfig: patchConfigFile,
  resetConfig: async () => {
    await resetConfigFile();
  },
  scanModules,
  saveModuleRules,
  getStorageUsage,
  getSystemInfo,
  getVersion,
  openLink,
  reboot,
} as AppAPI;

export { AppError } from "./api/core/error";
export { hasExecBridge, runDaemonCommand };
export type { AppAPI } from "./api/contracts";
export type { DaemonCommandPayload } from "./api/core/bridge";
const mockApi = shouldUseMock ? await loadMockApi() : null;
export const API: AppAPI = mockApi ?? RealAPI;
