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
  lkm: KasumiStatus["lkm"] | undefined;
  isExpanded: boolean;
  onToggle: () => void;
  runAction: RunAction;
  onShowKmiDialog: () => void;
  onShowUnloadWarning: () => void;
}

export interface RuntimeSectionProps {
  pending: boolean;
  config: KasumiStatus["config"] | undefined;
  status: KasumiStatus | null;
  lkm: KasumiStatus["lkm"] | undefined;
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
  config: KasumiStatus["config"] | undefined;
  isExpanded: boolean;
  onToggle: () => void;
  runAction: RunAction;
}

export interface FeaturesSectionProps {
  loading: boolean;
  status: KasumiStatus | null;
  config: KasumiStatus["config"] | undefined;
  activeModules: string[];
  isExpanded: boolean;
  onToggle: () => void;
}
