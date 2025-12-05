import { DEFAULT_CONFIG, PATHS } from './constants';
import { MockAPI } from './api.mock';
import type { AppConfig, Module, StorageStatus, SystemInfo, DeviceInfo } from './types';

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

const RealAPI = {
  loadConfig: async (): Promise<AppConfig> => {
    if (!ksuExec) return DEFAULT_CONFIG;
    const cmd = `${PATHS.BINARY} show-config`;
    try {
      const { errno, stdout } = await ksuExec(cmd);
      if (errno === 0 && stdout) {
        return JSON.parse(stdout);
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
    
    let bytes: Uint8Array;
    if (typeof TextEncoder !== 'undefined') {
      const encoder = new TextEncoder();
      bytes = encoder.encode(jsonStr);
    } else {
      bytes = new Uint8Array(jsonStr.length);
      for (let i = 0; i < jsonStr.length; i++) {
        bytes[i] = jsonStr.charCodeAt(i) & 0xFF;
      }
    }
    
    let hexPayload = '';
    for (let i = 0; i < bytes.length; i++) {
      const hex = bytes[i].toString(16);
      hexPayload += (hex.length === 1 ? '0' + hex : hex);
    }

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

  saveModules: async (modules: Module[]): Promise<void> => {
    if (!ksuExec) throw new Error("No KSU environment");
    let content = "# Module Modes\n";
    modules.forEach(m => { 
      if (m.mode !== 'auto' && /^[a-zA-Z0-9_.-]+$/.test(m.id)) {
        content += `${m.id}=${m.mode}\n`; 
      }
    });
    
    const data = content.replace(/'/g, "'\\''");
    const modeConfigPath = (PATHS as any).MODE_CONFIG || "/data/adb/meta-hybrid/module_mode.conf";
    const cmd = `mkdir -p "$(dirname "${modeConfigPath}")" && printf '%s\n' '${data}' > "${modeConfigPath}"`;
    
    const { errno } = await ksuExec(cmd);
    if (errno !== 0) throw new Error('Failed to save modes');
  },

  readLogs: async (logPath?: string, lines = 1000): Promise<string> => {
    if (!ksuExec) return "";
    const f = logPath || DEFAULT_CONFIG.logfile;
    const cmd = `[ -f "${f}" ] && tail -n ${lines} "${f}" || echo ""`;
    const { errno, stdout, stderr } = await ksuExec(cmd);
    
    if (errno === 0) return stdout || "";
    throw new Error(stderr || "Log file not found or unreadable");
  },

  getStorageUsage: async (): Promise<StorageStatus> => {
    if (!ksuExec) return { size: '-', used: '-', percent: '0%', type: null };
    try {
      const cmd = `${PATHS.BINARY} storage`;
      const { errno, stdout } = await ksuExec(cmd);
      
      if (errno === 0 && stdout) {
        return JSON.parse(stdout);
      }
    } catch (e) {
      console.error("Storage check failed:", e);
    }
    return { size: '-', used: '-', percent: '0%', type: null };
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

      const stateFile = PATHS.DAEMON_STATE || "/data/adb/meta-hybrid/run/daemon_state.json";
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
      }

      return info;
    } catch (e) {
      console.error("System info check failed:", e);
      return { kernel: 'Unknown', selinux: 'Unknown', mountBase: 'Unknown', activeMounts: [] };
    }
  },

  getDeviceStatus: async (): Promise<DeviceInfo> => {
    return { model: 'Device', android: '14', kernel: '-', selinux: '-' };
  },

  getVersion: async (): Promise<string> => {
    return "v1.0.0";
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
  }
};

export const API = shouldUseMock ? MockAPI : RealAPI;