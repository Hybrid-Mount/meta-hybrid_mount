import { AppError } from "./api/core/error";
import { PATHS } from "./constants";
import { ENABLE_KASUMI } from "./constants_gen";
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
  ...kasumiApi,
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
