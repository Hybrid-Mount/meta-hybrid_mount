import type {
  AppConfig,
  KernelUnameValues,
  KasumiStatus,
  KasumiUnameConfig,
  Module,
  ModuleRules,
  StorageStatus,
  SystemInfo,
} from "../types";

export interface InitPayload {
  status: unknown;
  config: unknown;
  version: string;
  kasumi_status?: unknown;
  system_info: unknown;
}

export interface AppAPI {
  wakeDaemon: () => Promise<void>;
  init: () => Promise<InitPayload>;
  loadConfig: () => Promise<AppConfig>;
  saveConfig: (config: AppConfig) => Promise<void>;
  resetConfig: () => Promise<void>;
  scanModules: (path?: string) => Promise<Module[]>;
  saveModules: (modules: Module[]) => Promise<void>;
  saveModuleRules: (moduleId: string, rules: ModuleRules) => Promise<void>;
  saveAllModuleRules: (rules: Record<string, ModuleRules>) => Promise<void>;
  getStorageUsage: () => Promise<StorageStatus>;
  getSystemInfo: () => Promise<SystemInfo>;
  getVersion: () => Promise<string>;
  getKasumiStatus: () => Promise<KasumiStatus>;
  setKasumiEnabled: (enabled: boolean) => Promise<void>;
  setKasumiStealth: (enabled: boolean) => Promise<void>;
  setKasumiHidexattr: (enabled: boolean) => Promise<void>;
  setKasumiSelinuxFix: (enabled: boolean) => Promise<void>;
  setKasumiDebug: (enabled: boolean) => Promise<void>;
  getOriginalKernelUname: () => Promise<KernelUnameValues>;
  setKasumiUnameMode: (mode: "scoped" | "global") => Promise<void>;
  setKasumiUname: (uname: Partial<KasumiUnameConfig>) => Promise<void>;
  applyKasumiUname: (
    mode: "scoped" | "global",
    uname: Pick<KasumiUnameConfig, "release" | "version">,
  ) => Promise<void>;
  clearKasumiUname: (mode?: "scoped" | "global") => Promise<void>;
  restoreKasumiUnameGlobal: () => Promise<void>;
  setKasumiCmdline: (value: string) => Promise<void>;
  clearKasumiCmdline: () => Promise<void>;
  addKasumiMapsRule: (rule: {
    target_ino: number;
    target_dev: number;
    spoofed_ino: number;
    spoofed_dev: number;
    spoofed_pathname: string;
  }) => Promise<void>;
  clearKasumiMapsRules: () => Promise<void>;
  getUserHideRules: () => Promise<string[]>;
  addUserHideRule: (path: string) => Promise<void>;
  removeUserHideRule: (path: string) => Promise<void>;
  applyUserHideRules: () => Promise<void>;
  loadKasumiLkm: () => Promise<void>;
  unloadKasumiLkm: () => Promise<void>;
  setKasumiLkmAutoload: (enabled: boolean) => Promise<void>;
  setKasumiLkmKmi: (value: string) => Promise<void>;
  clearKasumiLkmKmi: () => Promise<void>;
  fixKasumiMounts: () => Promise<void>;
  clearMountErrors: () => Promise<void>;
  clearKasumiRules: () => Promise<void>;
  releaseKasumiConnection: () => Promise<void>;
  invalidateKasumiCache: () => Promise<void>;
  openLink: (url: string) => Promise<void>;
  reboot: () => Promise<void>;
}
