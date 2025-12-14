import { DEFAULT_CONFIG, PATHS } from './constants';
import { APP_VERSION } from './constants_gen';
import { MockAPI } from './api.mock';
import type { AppConfig, Module, StorageStatus, SystemInfo, DeviceInfo, ModuleRules, ConflictEntry, DiagnosticIssue } from './types';

interface KsuExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

interface KsuModule {
  exec: (cmd: string, options?: any) => Promise<KsuExecResult>;
}

let ksuExec: KsuModule['exec'] | null = null;

try {
  const ksu = await import('kernelsu').catch(() => null);
  ksuExec = ksu ? ksu.exec : null;
} catch (e) {
  console.warn("KernelSU module not found, defaulting to Mock/Fallback.");
}

const shouldUseMock = import.meta.env.DEV || !ksuExec;
console.log(`[API Init] Mode: ${shouldUseMock ? '🛠️ MOCK (Dev/Browser)' : '🚀 REAL (Device)'}`);

function formatBytes(bytes: number, decimals = 2): string {
  if (!+bytes) return '0 B';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`;
}

function stringToHex(str: string): string {
  let bytes: Uint8Array;
  if (typeof TextEncoder !== 'undefined') {
    const encoder = new TextEncoder();
    bytes = encoder.encode(str);
  } else {
    bytes = new Uint8Array(str.length);
    for (let i = 0; i < str.length; i++) {
      bytes[i] = str.charCodeAt(i) & 0xFF;
    }
  }
  let hex = '';
  for (let i = 0; i < bytes.length; i++) {
    const h = bytes[i].toString(16);
    hex += (h.length === 1 ? '0' + h : h);
  }
  return hex;
}

const RealAPI = {
  loadConfig: async (): Promise<AppConfig> => {
    if (!ksuExec) return DEFAULT_CONFIG;
    const cmd = `${PATHS.BINARY} show-config`;
    try {
      const { errno, stdout } = await ksuExec(cmd);
      if (errno === 0 && stdout) {
        const loaded = JSON.parse(stdout);
        return { ...DEFAULT_CONFIG, ...loaded };
      } else {
        console.warn("Config load returned non-zero or empty, using defaults");
        return DEFAULT_CONFIG;
      }
    } catch (e) {
      console.error("Failed to load config from backend:", e);
      return DEFAULT_CONFIG; 
    }
  },
  saveConfig: async (config: AppConfig): Promise<void> => {
    if (!ksuExec) throw new Error("No KSU environment");
    const jsonStr = JSON.stringify(config);
    const hexPayload = stringToHex(jsonStr);
    const cmd = `${PATHS.BINARY} save-config --payload ${hexPayload}`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      throw new Error(`Failed to save config: ${stderr}`);
    }
  },
  scanModules: async (path?: string): Promise<Module[]> => {
    if (!ksuExec) return [];
    const cmd = `${PATHS.BINARY} modules`;
    try {
      const { errno, stdout } = await ksuExec(cmd);
      if (errno === 0 && stdout) {
        return JSON.parse(stdout);
      }
    } catch (e) {
      console.error("Module scan failed:", e);
    }
    return [];
  },
  saveModuleRules: async (moduleId: string, rules: ModuleRules): Promise<void> => {
    if (!ksuExec) throw new Error("No KSU environment");
    const jsonStr = JSON.stringify(rules);
    const hexPayload = stringToHex(jsonStr);
    const cmd = `${PATHS.BINARY} save-rules --module "${moduleId}" --payload "${hexPayload}"`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      throw new Error(`Failed to save rules for ${moduleId}: ${stderr}`);
    }
  },
  saveModules: async (modules: Module[]): Promise<void> => {
    return; 
  },
  readLogs: async (logPath?: string, lines = 1000): Promise<string> => {
    if (!ksuExec) return "";
    const f = logPath || (PATHS as any).DAEMON_LOG || "/data/adb/meta-hybrid/daemon.log";
    const cmd = `[ -f "${f}" ] && tail -n ${lines} "${f}" || echo ""`;
    const { errno, stdout, stderr } = await ksuExec(cmd);
    if (errno === 0) return stdout || "";
    throw new Error(stderr || "Log file not found or unreadable");
  },
  getStorageUsage: async (): Promise<StorageStatus> => {
    if (!ksuExec) return { size: '-', used: '-', percent: '0%', type: null, hymofs_available: false };
    try {
      const stateFile = (PATHS as any).DAEMON_STATE || "/data/adb/meta-hybrid/run/daemon_state.json";
      const cmd = `cat "${stateFile}"`;
      const { errno, stdout } = await ksuExec(cmd);
      if (errno === 0 && stdout) {
        const state = JSON.parse(stdout);
        return {
          type: state.storage_mode || 'unknown',
          percent: `${state.storage_percent ?? 0}%`,
          size: formatBytes(state.storage_total ?? 0),
          used: formatBytes(state.storage_used ?? 0),
          hymofs_available: state.hymofs_available ?? false,
          hymofs_version: state.hymofs_version
        };
      }
    } catch (e) {
      console.error("Storage check failed:", e);
    }
    return { size: '-', used: '-', percent: '0%', type: null, hymofs_available: false };
  },
  getSystemInfo: async (): Promise<SystemInfo> => {
    if (!ksuExec) return { kernel: 'Unknown', selinux: 'Unknown', mountBase: 'Unknown', activeMounts: [] };
    try {
      const cmdSys = `echo "KERNEL:$(uname -r)"; echo "SELINUX:$(getenforce)"`;
      const { errno: errSys, stdout: outSys } = await ksuExec(cmdSys);
      let info: SystemInfo = { kernel: '-', selinux: '-', mountBase: '-', activeMounts: [] };
      if (errSys === 0 && outSys) {
        outSys.split('\n').forEach(line => {
          if (line.startsWith('KERNEL:')) info.kernel = line.substring(7).trim();
          else if (line.startsWith('SELINUX:')) info.selinux = line.substring(8).trim();
        });
      }
      
      const cmdZygisk = `[ -f "/data/adb/zygisksu/denylist_enforce" ] && cat "/data/adb/zygisksu/denylist_enforce" || echo "0"`;
      const { errno: errZygisk, stdout: outZygisk } = await ksuExec(cmdZygisk);
      if (errZygisk === 0) {
          info.zygisksuEnforce = outZygisk.trim();
      }

      const stateFile = (PATHS as any).DAEMON_STATE || "/data/adb/meta-hybrid/run/daemon_state.json";
      const cmdState = `cat "${stateFile}"`;
      const { errno: errState, stdout: outState } = await ksuExec(cmdState);
      if (errState === 0 && outState) {
        try {
          const state = JSON.parse(outState);
          info.mountBase = state.mount_point || 'Unknown';
          if (Array.isArray(state.active_mounts)) {
            info.activeMounts = state.active_mounts;
          }
        } catch (e) {
          console.error("Failed to parse daemon state JSON", e);
        }
      } else {
          const mntPath = (PATHS as any).IMAGE_MNT || "/data/adb/meta-hybrid/img_mnt";
          const m = await ksuExec(`mount | grep "${mntPath}" | head -n 1`);
          if (m.errno === 0 && m.stdout) {
              const parts = m.stdout.split(' ');
              if (parts.length > 2) info.mountBase = parts[2]; 
          }
      }
      return info;
    } catch (e) {
      console.error("System info check failed:", e);
      return { kernel: 'Unknown', selinux: 'Unknown', mountBase: 'Unknown', activeMounts: [] };
    }
  },
  getDeviceStatus: async (): Promise<DeviceInfo> => {
    let model = "Device";
    let android = "14";
    let kernel = "Unknown";
    if (ksuExec) {
        const p1 = await ksuExec('getprop ro.product.model');
        if (p1.errno === 0) model = p1.stdout.trim();
        const p2 = await ksuExec('getprop ro.build.version.release');
        const p3 = await ksuExec('getprop ro.build.version.sdk');
        if (p2.errno === 0) android = `${p2.stdout.trim()} (API ${p3.stdout.trim()})`;
        const p4 = await ksuExec('uname -r');
        if (p4.errno === 0) kernel = p4.stdout.trim();
    }
    return {
        model,
        android,
        kernel,
        selinux: "Enforcing"
    };
  },
  getVersion: async (): Promise<string> => {
    if (!ksuExec) return APP_VERSION;
    try {
        const binPath = PATHS.BINARY;
        const moduleDir = binPath.substring(0, binPath.lastIndexOf('/'));
        const propPath = `${moduleDir}/module.prop`;
        const cmd = `grep "^version=" "${propPath}"`;
        const { errno, stdout } = await ksuExec(cmd);
        if (errno === 0 && stdout) {
            const match = stdout.match(/^version=(.+)$/m);
            if (match && match[1]) {
                return match[1].trim();
            }
        }
    } catch (e) {
        console.error("Failed to read module version", e);
    }
    return APP_VERSION;
  },
  openLink: async (url: string): Promise<void> => {
    if (!ksuExec) {
        window.open(url, '_blank');
        return;
    }
    const safeUrl = url.replace(/"/g, '\\"');
    const cmd = `am start -a android.intent.action.VIEW -d "${safeUrl}"`;
    await ksuExec(cmd);
  },
  fetchSystemColor: async (): Promise<string | null> => {
    if (!ksuExec) return null;
    try {
      const { stdout } = await ksuExec('settings get secure theme_customization_overlay_packages');
      if (stdout) {
        const match = /["']?android\.theme\.customization\.system_palette["']?\s*:\s*["']?#?([0-9a-fA-F]{6,8})["']?/i.exec(stdout) || 
                      /["']?source_color["']?\s*:\s*["']?#?([0-9a-fA-F]{6,8})["']?/i.exec(stdout);
        if (match && match[1]) {
          let hex = match[1];
          if (hex.length === 8) hex = hex.substring(2);
          return '#' + hex;
        }
      }
    } catch (e) {}
    return null;
  },
  getConflicts: async (): Promise<ConflictEntry[]> => {
    if (!ksuExec) return [];
    const cmd = `${PATHS.BINARY} conflicts`;
    try {
        const { errno, stdout } = await ksuExec(cmd);
        if (errno === 0 && stdout) {
            return JSON.parse(stdout);
        }
    } catch(e) {
        console.error("Failed to get conflicts:", e);
    }
    return [];
  },
  getDiagnostics: async (): Promise<DiagnosticIssue[]> => {
      if (!ksuExec) return [];
      const cmd = `${PATHS.BINARY} diagnostics`;
      try {
          const { errno, stdout } = await ksuExec(cmd);
          if (errno === 0 && stdout) {
              return JSON.parse(stdout);
          }
      } catch(e) {
          console.error("Failed to get diagnostics:", e);
      }
      return [];
  },
  reboot: async (): Promise<void> => {
    if (!ksuExec) return;
    try {
        await ksuExec('reboot');
    } catch (e) {
        console.error("Reboot failed", e);
    }
  }
};

export const API = shouldUseMock ? MockAPI : RealAPI;