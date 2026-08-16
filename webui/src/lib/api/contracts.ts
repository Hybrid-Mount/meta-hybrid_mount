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
  Module,
  ModuleRules,
  StorageStatus,
  SystemInfo,
} from "../types";

export interface InitPayload {
  status: unknown;
  config: unknown;
  version: string;
  system_info: unknown;
}

export type InstallState = "ready" | "clean-reinstall-required";

export interface AppAPI {
  checkInstallState: () => Promise<InstallState>;
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
  clearMountErrors: () => Promise<void>;
  openLink: (url: string) => Promise<void>;
  reboot: () => Promise<void>;
}
