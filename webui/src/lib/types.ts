export type MountMode = "overlay" | "magic" | "ignore" | "hymofs";

export type OverlayMode = "tmpfs" | "ext4" | "erofs";

export interface ModuleRules {
  default_mode: MountMode;
  paths: Record<string, string>;
}

export interface HymofsConfig {
  enable: boolean;
  debug: boolean;
  stealth: boolean;
  hide_overlay_xattrs: boolean;
}

export interface AppConfig {
  moduledir: string;
  mountsource: string;
  partitions: string[];
  overlay_mode: OverlayMode;
  disable_umount: boolean;
  allow_umount_coexistence: boolean;
  logfile?: string;
  hymofs: HymofsConfig;
}

export interface Module {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  mode: string;
  is_mounted: boolean;
  enabled?: boolean;
  source_path?: string;
  rules: ModuleRules;
}

export interface StorageStatus {
  type: "tmpfs" | "ext4" | "erofs" | "unknown" | null;
  error?: string;
}

export interface HymoFSState {
  enabled: boolean;
  loaded: boolean;
  version: number;
  active_features: string[];
  error_msg: string | null;
}

export interface SystemInfo {
  kernel: string;
  selinux: string;
  mountBase: string;
  activeMounts: string[];
  zygisksuEnforce?: string;
  supported_overlay_modes?: OverlayMode[];
  tmpfs_xattr_supported?: boolean;
  abi?: string;
  hymofs_state?: HymoFSState;
}

export interface DeviceInfo {
  model: string;
  android: string;
  kernel: string;
  selinux: string;
}

export interface ToastMessage {
  id: string;
  text: string;
  type: "info" | "success" | "error";
  visible: boolean;
}

export interface LanguageOption {
  code: string;
  name: string;
  display?: string;
}

export interface ModeStats {
  auto: number;
  magic: number;
  hymofs: number;
}
