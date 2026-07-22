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

import type { KasumiStatus } from "../../lib/types";

export type RefreshMode = "status-only" | "full";

export type RunAction = (
  action: () => Promise<void>,
  success: string,
  refreshMode?: RefreshMode,
) => Promise<void>;

export interface SectionShellProps {
  id: string;
  title: string;
  isExpanded: boolean;
  onToggle: () => void;
  badge?: string;
  badgeActive?: boolean;
  children: import("solid-js").JSXElement;
}

export interface LkmSectionProps {
  pending: boolean;
  kmi: string;
  setKmi: (v: string) => void;
  lkm: KasumiStatus["lkm"];
  isExpanded: boolean;
  onToggle: () => void;
  runAction: RunAction;
  onShowKmiDialog: () => void;
  onShowUnloadWarning: () => void;
}

export interface RuntimeSectionProps {
  pending: boolean;
  config: KasumiStatus["config"];
  status: KasumiStatus;
  lkm: KasumiStatus["lkm"];
  isExpanded: boolean;
  onToggle: () => void;
  runAction: RunAction;
}

export interface IdentitySectionProps {
  pending: boolean;
  unameMode: "scoped" | "global";
  setUnameMode: (v: "scoped" | "global") => void;
  release: string;
  setRelease: (v: string) => void;
  version: string;
  setVersion: (v: string) => void;
  cmdline: string;
  setCmdline: (v: string) => void;
  unameModeDescription: string;
  isExpanded: boolean;
  onToggle: () => void;
  runAction: RunAction;
  fillOriginalKernelUname: () => Promise<void>;
  saveAndApplyUname: () => Promise<void>;
  clearUname: () => Promise<void>;
}

export interface UserHideSectionProps {
  pending: boolean;
  userHidePath: string;
  setUserHidePath: (v: string) => void;
  userHideRules: string[];
  isExpanded: boolean;
  onToggle: () => void;
  runAction: RunAction;
}

export interface MapsSectionProps {
  pending: boolean;
  mapsTargetIno: string;
  setMapsTargetIno: (v: string) => void;
  mapsTargetDev: string;
  setMapsTargetDev: (v: string) => void;
  mapsSpoofedIno: string;
  setMapsSpoofedIno: (v: string) => void;
  mapsSpoofedDev: string;
  setMapsSpoofedDev: (v: string) => void;
  mapsPath: string;
  setMapsPath: (v: string) => void;
  config: KasumiStatus["config"];
  isExpanded: boolean;
  onToggle: () => void;
  runAction: RunAction;
}

export interface FeaturesSectionProps {
  loading: boolean;
  status: KasumiStatus;
  config: KasumiStatus["config"];
  activeModules: string[];
  isExpanded: boolean;
  onToggle: () => void;
}
