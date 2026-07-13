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
  patchConfig: (
    patch: Record<string, unknown>,
    options?: { applyRuntime?: boolean },
  ) => Promise<AppConfig>;
  resetConfig: () => Promise<void>;
  scanModules: (path?: string) => Promise<Module[]>;
  saveModuleRules: (moduleId: string, rules: ModuleRules) => Promise<void>;
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
