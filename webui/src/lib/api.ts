import { DEFAULT_CONFIG, PATHS } from "./constants";
import { APP_VERSION } from "./constants_gen";
import { MockAPI } from "./api.mock";
import type {
  AppConfig,
  Module,
  StorageStatus,
  SystemInfo,
  DeviceInfo,
  ModuleRules,
} from "./types";

interface KsuExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

interface KsuModule {
  exec: (cmd: string, options?: unknown) => Promise<KsuExecResult>;
}

let ksuExec: KsuModule["exec"] | null = null;

try {
  const ksu = await import("kernelsu").catch(() => null);
  ksuExec = ksu ? ksu.exec : null;
} catch {
  console.warn("KernelSU module not found, defaulting to Mock/Fallback.");
}

const shouldUseMock = import.meta.env.DEV || !ksuExec;

const OVERLAY_COLOR_MAP: Record<string, string> = {
  "com.android.theme.color.cinnamon": "#9F6047",
  "com.android.theme.color.black": "#3C3F41",
  "com.android.theme.color.green": "#3DDC84",
  "com.android.theme.color.ocean": "#009688",
  "com.android.theme.color.space": "#475975",
  "com.android.theme.color.orchid": "#DA70D6",
  "com.android.theme.color.purple": "#9C27B0",
  "org.lineageos.overlay.accent.blue": "#4285F4",
  "org.lineageos.overlay.accent.cyan": "#00BCD4",
  "org.lineageos.overlay.accent.green": "#4CAF50",
  "org.lineageos.overlay.accent.orange": "#FF9800",
  "org.lineageos.overlay.accent.pink": "#E91E63",
  "org.lineageos.overlay.accent.purple": "#9C27B0",
  "org.lineageos.overlay.accent.red": "#F44336",
  "org.lineageos.overlay.accent.yellow": "#FFEB3B",
};

function stringToHex(str: string): string {
  let bytes: Uint8Array;
  if (typeof TextEncoder !== "undefined") {
    const encoder = new TextEncoder();
    bytes = encoder.encode(str);
  } else {
    bytes = new Uint8Array(str.length);
    for (let i = 0; i < str.length; i++) {
      bytes[i] = str.charCodeAt(i) & 0xff;
    }
  }
  let hex = "";
  for (let i = 0; i < bytes.length; i++) {
    const h = bytes[i].toString(16);
    hex += h.length === 1 ? "0" + h : h;
  }
  return hex;
}

interface AppAPI {
  loadConfig: () => Promise<AppConfig>;
  saveConfig: (config: AppConfig) => Promise<void>;
  resetConfig: () => Promise<void>;
  scanModules: (path?: string) => Promise<Module[]>;
  saveModules: (modules: Module[]) => Promise<void>;
  saveModuleRules: (moduleId: string, rules: ModuleRules) => Promise<void>;
  getStorageUsage: () => Promise<StorageStatus>;
  getSystemInfo: () => Promise<SystemInfo>;
  getDeviceStatus: () => Promise<DeviceInfo>;
  getVersion: () => Promise<string>;
  openLink: (url: string) => Promise<void>;
  fetchSystemColor: () => Promise<string | null>;
  reboot: () => Promise<void>;
  readLogs: () => Promise<string>;
}

const RealAPI: AppAPI = {
  loadConfig: async (): Promise<AppConfig> => {
    if (!ksuExec) return DEFAULT_CONFIG;
    const cmd = `${PATHS.BINARY} show-config`;
    try {
      const { errno, stdout } = await ksuExec(cmd);
      if (errno === 0 && stdout) {
        const loaded = JSON.parse(stdout);
        return { ...DEFAULT_CONFIG, ...loaded };
      }
    } catch {}
    return DEFAULT_CONFIG;
  },
  saveConfig: async (config: AppConfig): Promise<void> => {
    if (!ksuExec) throw new Error("No KSU environment");
    const jsonStr = JSON.stringify(config);
    const hexPayload = stringToHex(jsonStr);
    const cmd = `${PATHS.BINARY} save-config --payload ${hexPayload}`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) throw new Error(`Failed to save config: ${stderr}`);
  },
  resetConfig: async (): Promise<void> => {
    if (!ksuExec) throw new Error("No KSU environment");
    const cmd = `${PATHS.BINARY} gen-config`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) throw new Error(`Failed to reset config: ${stderr}`);
  },
  scanModules: async (_path?: string): Promise<Module[]> => {
    if (!ksuExec) return [];
    const cmd = `${PATHS.BINARY} modules`;
    try {
      const { errno, stdout } = await ksuExec(cmd);
      if (errno === 0 && stdout) return JSON.parse(stdout);
    } catch {}
    return [];
  },
  saveModules: async (_modules: Module[]): Promise<void> => {
    return;
  },
  readLogs: async (): Promise<string> => {
    if (!ksuExec) return "";
    try {
      const { errno, stdout } = await ksuExec(
        `cat "${DEFAULT_CONFIG.logfile}"`,
      );
      if (errno === 0 && stdout) return stdout;
    } catch (e) {}
    return "";
  },

  saveModuleRules: async (
    moduleId: string,
    rules: ModuleRules,
  ): Promise<void> => {
    if (!ksuExec) throw new Error("No KSU environment");
    const jsonStr = JSON.stringify(rules);
    const hexPayload = stringToHex(jsonStr);
    const cmd = `${PATHS.BINARY} save-module-rules --module "${moduleId}" --payload ${hexPayload}`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) throw new Error(`Failed to save rules: ${stderr}`);
  },
  getStorageUsage: async (): Promise<StorageStatus> => {
    if (!ksuExec) return { type: null };
    try {
      const stateFile =
        (PATHS as Record<string, string>).DAEMON_STATE ||
        "/data/adb/meta-hybrid/run/daemon_state.json";
      const { errno, stdout } = await ksuExec(`cat "${stateFile}"`);
      if (errno === 0 && stdout) {
        const state = JSON.parse(stdout);
        return {
          type: state.storage_mode || "unknown",
        };
      }
    } catch {}
    return { type: null };
  },
  getSystemInfo: async (): Promise<SystemInfo> => {
    if (!ksuExec)
      return { kernel: "-", selinux: "-", mountBase: "-", activeMounts: [] };
    try {
      const cmdSys = `echo "KERNEL:$(uname -r)"; echo "SELINUX:$(getenforce)"`;
      const { errno: errSys, stdout: outSys } = await ksuExec(cmdSys);
      const info: SystemInfo = {
        kernel: "-",
        selinux: "-",
        mountBase: "-",
        activeMounts: [],
      };
      if (errSys === 0 && outSys) {
        outSys.split("\n").forEach((line) => {
          if (line.startsWith("KERNEL:"))
            info.kernel = line.substring(7).trim();
          else if (line.startsWith("SELINUX:"))
            info.selinux = line.substring(8).trim();
        });
      }
      const stateFile =
        (PATHS as Record<string, string>).DAEMON_STATE ||
        "/data/adb/meta-hybrid/run/daemon_state.json";
      const { errno: errState, stdout: outState } = await ksuExec(
        `cat "${stateFile}"`,
      );
      if (errState === 0 && outState) {
        try {
          const state = JSON.parse(outState);
          info.mountBase = state.mount_point || "Unknown";
          info.activeMounts = state.active_mounts || [];
          if (state.zygisksu_enforce !== undefined) {
            info.zygisksuEnforce = state.zygisksu_enforce ? "1" : "0";
          }
          if (state.tmpfs_xattr_supported !== undefined) {
            info.tmpfs_xattr_supported = state.tmpfs_xattr_supported;
          }
        } catch {}
      }
      return info;
    } catch {
      return { kernel: "-", selinux: "-", mountBase: "-", activeMounts: [] };
    }
  },
  getDeviceStatus: async (): Promise<DeviceInfo> => {
    let model = "Device",
      android = "14",
      kernel = "Unknown";
    if (ksuExec) {
      try {
        const p1 = await ksuExec("getprop ro.product.model");
        if (p1.errno === 0) model = p1.stdout.trim();
        const p2 = await ksuExec("getprop ro.build.version.release");
        const p3 = await ksuExec("getprop ro.build.version.sdk");
        if (p2.errno === 0)
          android = `${p2.stdout.trim()} (API ${p3.stdout.trim()})`;
        const p4 = await ksuExec("uname -r");
        if (p4.errno === 0) kernel = p4.stdout.trim();
      } catch {}
    }
    return { model, android, kernel, selinux: "Enforcing" };
  },
  getVersion: async (): Promise<string> => {
    if (!ksuExec) return APP_VERSION;
    try {
      const binPath = PATHS.BINARY;
      const moduleDir = binPath.substring(0, binPath.lastIndexOf("/"));
      const { errno, stdout } = await ksuExec(
        `grep "^version=" "${moduleDir}/module.prop"`,
      );
      if (errno === 0 && stdout) {
        const match = stdout.match(/^version=(.+)$/m);
        if (match) return match[1].trim();
      }
    } catch {}
    return APP_VERSION;
  },
  openLink: async (url: string): Promise<void> => {
    if (!ksuExec) {
      window.open(url, "_blank");
      return;
    }
    await ksuExec(
      `am start -a android.intent.action.VIEW -d "${url.replace(/"/g, '\\"')}"`,
    );
  },
  fetchSystemColor: async (): Promise<string | null> => {
    if (!ksuExec) return null;
    try {
      const { stdout: settingsOut } = await ksuExec(
        "settings get secure theme_customization_overlay_packages",
      );
      if (settingsOut) {
        const match =
          /["']?(?:android\.theme\.customization\.system_palette|source_color)["']?\s*:\s*["']?#?([0-9a-fA-F]{6,8})["']?/i.exec(
            settingsOut,
          );
        if (match?.[1]) {
          const hex = match[1].length === 8 ? match[1].substring(2) : match[1];
          return `#${hex}`;
        }
      }

      const { stdout: dumpOut } = await ksuExec(
        "dumpsys wallpaper | grep -E 'mMainColor|mPrimaryColors|mConnection'",
      );
      if (dumpOut) {
        const match = /mMainColor=0x([0-9a-fA-F]{8})/i.exec(dumpOut);
        if (match?.[1]) {
          const hex = match[1].length === 8 ? match[1].substring(2) : match[1];
          return `#${hex}`;
        }
      }

      const { stdout: overlayOut } = await ksuExec(
        "cmd overlay list --user current",
      );
      if (overlayOut) {
        const lines = overlayOut.split("\n");
        for (const line of lines) {
          if (line.includes("[x]")) {
            for (const [pkg, color] of Object.entries(OVERLAY_COLOR_MAP)) {
              if (line.includes(pkg)) {
                return color;
              }
            }
          }
        }
      }

      const { stdout: propOut } = await ksuExec(
        "getprop persist.sys.theme.color",
      );
      if (propOut) {
        const trimmed = propOut.trim();
        if (/^#?[0-9a-fA-F]{6,8}$/.test(trimmed)) {
          return trimmed.startsWith("#") ? trimmed : `#${trimmed}`;
        }
      }
    } catch {}
    return null;
  },
  reboot: async (): Promise<void> => {
    if (!ksuExec) return;
    await ksuExec("reboot");
  },
};

export const API: AppAPI = shouldUseMock
  ? (MockAPI as unknown as AppAPI)
  : RealAPI;
