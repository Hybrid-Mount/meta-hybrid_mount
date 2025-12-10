import { APP_VERSION } from './constants_gen';
import { DEFAULT_CONFIG } from './constants';
import type { AppConfig, DeviceInfo, Module, StorageStatus, SystemInfo, ModuleRules } from './types';

// Mock delay to simulate network latency
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

export const MockAPI = {
  async loadConfig(): Promise<AppConfig> {
    await delay(300);
    return { ...DEFAULT_CONFIG };
  },

  async saveConfig(config: AppConfig): Promise<void> {
    await delay(500);
    console.log('[Mock] Config saved:', config);
  },

  async scanModules(dir: string): Promise<Module[]> {
    await delay(600);
    return [
      {
        id: 'magisk_module_1',
        name: 'Example Module',
        version: '1.0.0',
        author: 'Developer',
        description: 'This is a mock module for testing.',
        mode: 'magic',
        rules: { 
            default_mode: 'magic', 
            paths: { "system/fonts": "hymofs" } 
        }
      },
      {
        id: 'overlay_module_2',
        name: 'System UI Overlay',
        version: '2.5',
        author: 'Google',
        description: 'Changes system colors.',
        mode: 'auto',
        rules: { 
            default_mode: 'overlay', 
            paths: {} 
        }
      }
    ];
  },

  async saveModuleRules(moduleId: string, rules: ModuleRules): Promise<void> {
    await delay(400);
    console.log(`[Mock] Rules saved for ${moduleId}:`, rules);
  },

  // Deprecated but kept for interface compatibility if strictly typed elsewhere
  async saveModules(modules: Module[]): Promise<void> {
    console.warn("[Mock] saveModules is deprecated");
  },

  async readLogs(): Promise<string> {
    await delay(200);
    return `[I] Daemon started at ${new Date().toISOString()}
[I] Loading config from /data/adb/meta-hybrid/config.toml
[D] Scanning modules...
[I] Found 2 modules
[W] OverlayFS is not supported on this kernel, falling back to Magic Mount
[E] Failed to mount /system/app/TestApp: No such file or directory
[I] Daemon ready`;
  },

  async getDeviceStatus(): Promise<DeviceInfo> {
    await delay(300);
    return {
      model: 'Pixel 8 Pro (Mock)',
      android: '14 (API 34)',
      kernel: '5.15.110-android14-11',
      selinux: 'Enforcing'
    };
  },

  async getVersion(): Promise<string> {
    await delay(100);
    return APP_VERSION;
  },

  async getStorageUsage(): Promise<StorageStatus> {
    await delay(300);
    return {
      used: '128 MB',
      size: '1024 MB',
      percent: '12.5%',
      type: 'tmpfs',
      hymofs_available: true
    };
  },

  async getSystemInfo(): Promise<SystemInfo> {
    await delay(300);
    return {
      kernel: 'Linux localhost 5.15.0 #1 SMP PREEMPT',
      selinux: 'Enforcing',
      mountBase: '/data/adb/meta-hybrid/mnt',
      activeMounts: ['system', 'product']
    };
  },

  async fetchSystemColor(): Promise<string | null> {
    await delay(100);
    return '#8AB4F8';
  },

  openLink(url: string): void {
    console.log('[Mock] Opening link:', url);
    window.open(url, '_blank');
  }
};