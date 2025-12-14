export interface AppConfig {
  moduledir: string;
  tempdir: string;
  mountsource: string;
  verbose: boolean;
  partitions: string[];
  force_ext4: boolean;
  enable_nuke: boolean;
  disable_umount: boolean;
  allow_umount_coexistence: boolean;
  dry_run: boolean;
  logfile?: string;
}

export type MountMode = 'overlay' | 'hymofs' | 'magic' | 'ignore';

export interface ModuleRules {
  default_mode: MountMode;
  paths: Record<string, MountMode>;
}

export interface Module {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  mode: string;
  is_mounted: boolean;
  rules: ModuleRules;
  enabled?: boolean;
  source_path?: string;
}

export interface StorageStatus {
  size: string;
  used: string;
  percent: string;
  type: 'tmpfs' | 'ext4' | 'unknown' | null;
  error?: string;
  hymofs_available: boolean;
  hymofs_version?: number;
}

export interface SystemInfo {
  kernel: string;
  selinux: string;
  mountBase: string;
  activeMounts: string[];
  zygisksuEnforce?: string;
  nukeActive?: boolean;
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
  type: 'info' | 'success' | 'error';
  visible: boolean;
}

export interface LanguageOption {
  code: string;
  name: string;
}

export interface ModeStats {
  auto: number;
  magic: number;
  hymofs: number;
}

export interface ConflictEntry {
  partition: string;
  relative_path: string;
  contending_modules: string[];
}

export interface DiagnosticIssue {
  level: 'Info' | 'Warning' | 'Critical';
  context: string;
  message: string;
}
