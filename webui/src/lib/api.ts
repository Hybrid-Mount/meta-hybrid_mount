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
import { ENABLE_KASUMI } from "./constants_gen";
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
  saveConfigToFile,
} from "./api/repos/configRepo";
import { scanModules, saveModuleRules } from "./api/services/moduleService";
import type { AppAPI } from "./api/contracts";

function loadKasumiService() {
  if (!ENABLE_KASUMI) {
    return Promise.reject(
      new AppError("Kasumi is not available in this build"),
    );
  }
  return import("./api/services/kasumiService");
}

const kasumiApi: Partial<AppAPI> = ENABLE_KASUMI
  ? {
      getKasumiStatus: async () =>
        (await loadKasumiService()).getKasumiStatus(),
      setKasumiEnabled: async (enabled: boolean) =>
        (await loadKasumiService()).setKasumiEnabled(enabled),
      setKasumiStealth: async (enabled: boolean) =>
        (await loadKasumiService()).setKasumiStealth(enabled),
      setKasumiHidexattr: async (enabled: boolean) =>
        (await loadKasumiService()).setKasumiHidexattr(enabled),
      setKasumiSelinuxFix: async (enabled: boolean) =>
        (await loadKasumiService()).setKasumiSelinuxFix(enabled),
      setKasumiDebug: async (enabled: boolean) =>
        (await loadKasumiService()).setKasumiDebug(enabled),
      getOriginalKernelUname: async () =>
        (await loadKasumiService()).getOriginalKernelUname(),
      setKasumiUnameMode: async (mode: "scoped" | "global") =>
        (await loadKasumiService()).setKasumiUnameMode(mode),
      setKasumiUname: async (uname) =>
        (await loadKasumiService()).setKasumiUname(uname),
      applyKasumiUname: async (mode, uname) =>
        (await loadKasumiService()).applyKasumiUname(mode, uname),
      clearKasumiUname: async (mode = "scoped") =>
        (await loadKasumiService()).clearKasumiUname(mode),
      restoreKasumiUnameGlobal: async () =>
        (await loadKasumiService()).restoreKasumiUnameGlobal(),
      setKasumiCmdline: async (value: string) =>
        (await loadKasumiService()).setKasumiCmdline(value),
      clearKasumiCmdline: async () =>
        (await loadKasumiService()).clearKasumiCmdline(),
      addKasumiMapsRule: async (rule) =>
        (await loadKasumiService()).addKasumiMapsRule(rule),
      clearKasumiMapsRules: async () =>
        (await loadKasumiService()).clearKasumiMapsRules(),
      getUserHideRules: async () =>
        (await loadKasumiService()).getUserHideRules(),
      addUserHideRule: async (path: string) =>
        (await loadKasumiService()).addUserHideRule(path),
      removeUserHideRule: async (path: string) =>
        (await loadKasumiService()).removeUserHideRule(path),
      applyUserHideRules: async () =>
        (await loadKasumiService()).applyUserHideRules(),
      loadKasumiLkm: async () => (await loadKasumiService()).loadKasumiLkm(),
      unloadKasumiLkm: async () =>
        (await loadKasumiService()).unloadKasumiLkm(),
      setKasumiLkmAutoload: async (enabled: boolean) =>
        (await loadKasumiService()).setKasumiLkmAutoload(enabled),
      setKasumiLkmKmi: async (value: string) =>
        (await loadKasumiService()).setKasumiLkmKmi(value),
      clearKasumiLkmKmi: async () =>
        (await loadKasumiService()).clearKasumiLkmKmi(),
      fixKasumiMounts: async () =>
        (await loadKasumiService()).fixKasumiMounts(),
      clearKasumiRules: async () =>
        (await loadKasumiService()).clearKasumiRules(),
      releaseKasumiConnection: async () =>
        (await loadKasumiService()).releaseKasumiConnection(),
      invalidateKasumiCache: async () =>
        (await loadKasumiService()).invalidateKasumiCache(),
    }
  : {};

const RealAPI = {
  wakeDaemon: () => ensureDaemonAwake(PATHS.BINARY),
  init,
  loadConfig: loadConfigFromFile,
  saveConfig: saveConfigToFile,
  patchConfig: patchConfigFile,
  resetConfig: async () => {
    await resetConfigFile();
  },
  scanModules,
  saveModuleRules,
  getStorageUsage,
  getSystemInfo,
  getVersion,
  clearMountErrors: () =>
    runDaemonCommand(
      { type: "clear-mount-errors" },
      PATHS.BINARY,
    ) as Promise<void>,
  ...kasumiApi,
  openLink,
  reboot,
} as AppAPI;

export { AppError, hasExecBridge, runDaemonCommand };
export type { AppAPI } from "./api/contracts";
export type { DaemonCommandPayload } from "./api/core/bridge";
const mockApi = shouldUseMock ? await loadMockApi() : null;
export const API: AppAPI = mockApi ?? RealAPI;
