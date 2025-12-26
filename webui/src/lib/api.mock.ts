/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { APP_VERSION } from './constants_gen';
import { DEFAULT_CONFIG } from './constants';
import type { AppConfig, DeviceInfo, Module, StorageStatus, SystemInfo, ModuleRules, ConflictEntry, DiagnosticIssue, Silo } from './types';

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));


let mockSilos: Silo[] = [
    { 
        id: "silo_1715000000", 
        timestamp: 1715000000, 
        label: "Boot Backup", 
        reason: "Automatic Pre-Mount",
        config_snapshot: { ...DEFAULT_CONFIG }
    },
    { 
        id: "silo_1715003600", 
        timestamp: 1715003600, 
        label: "Manual Save", 
        reason: "User Action",
        config_snapshot: { ...DEFAULT_CONFIG }
    }
];

export const MockAPI = {
  async loadConfig(): Promise<AppConfig> {
    await delay(300);
    return { ...DEFAULT_CONFIG };
  },
  async saveConfig(config: AppConfig): Promise<void> {
    await delay(500);
    console.log('[Mock] Config saved:', config);
  },
  async resetConfig(): Promise<void> {
    await delay(500);
    console.log('[Mock] Config reset to defaults');
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
        is_mounted: true,
        rules: { 
            default_mode: 'magic', 
            paths: { "system/fonts": "overlay" } 
        }
      },
      {
        id: 'overlay_module_2',
        name: 'System UI Overlay',
        version: '2.5',
        author: 'Google',
        description: 'Changes system colors.',
        mode: 'auto',
        is_mounted: true,
        rules: { 
            default_mode: 'overlay', 
            paths: {} 
        }
      },
      {
        id: 'disabled_module',
        name: 'Unmounted Module',
        version: '0.1',
        author: 'Tester',
        description: 'This module is not mounted.',
        mode: 'ignore',
        is_mounted: false,
        rules: {
            default_mode: 'ignore',
            paths: {}
        }
      }
    ];
  },
  async saveModuleRules(moduleId: string, rules: ModuleRules): Promise<void> {
    await delay(400);
    console.log(`[Mock] Rules saved for ${moduleId}:`, rules);
  },
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
      type: 'erofs',
      hymofs_available: true,
    };
  },
  async getSystemInfo(): Promise<SystemInfo> {
    await delay(300);
    return {
      kernel: 'Linux localhost 5.15.0 #1 SMP PREEMPT',
      selinux: 'Enforcing',
      mountBase: '/data/adb/meta-hybrid/mnt',
      activeMounts: ['system', 'product'],
      zygisksuEnforce: '1'
    };
  },
  async fetchSystemColor(): Promise<string | null> {
    await delay(100);
    return '#8AB4F8';
  },
  async getConflicts(): Promise<ConflictEntry[]> {
    await delay(500);
    return [
    ];
  },
  async getDiagnostics(): Promise<DiagnosticIssue[]> {
      await delay(500);
      return [
          { level: "Info", context: "System", message: "OverlayFS is available" },
          { level: "Warning", context: "magisk_module_1", message: "Dead absolute symlink: system/bin/test -> /dev/null" }
      ];
  },

  async getGranaryList(): Promise<Silo[]> {
    await delay(400);
    return JSON.parse(JSON.stringify(mockSilos));
  },
  async createSilo(reason: string): Promise<void> {
    await delay(500);
    const newSilo: Silo = {
        id: `silo_${Math.floor(Date.now() / 1000)}`,
        timestamp: Math.floor(Date.now() / 1000),
        label: "Manual Snapshot",
        reason: reason,
        config_snapshot: { ...DEFAULT_CONFIG }
    };
    mockSilos.unshift(newSilo);
    console.log('[Mock] Created silo:', newSilo);
  },
  async deleteSilo(siloId: string): Promise<void> {
    await delay(500);
    mockSilos = mockSilos.filter(s => s.id !== siloId);
    console.log(`[Mock] Deleted silo: ${siloId}`);
  },
  async restoreSilo(siloId: string): Promise<void> {
    await delay(500);
    console.log(`[Mock] Restored silo: ${siloId}`);
  },

  async setWinnowingRule(path: string, moduleId: string): Promise<void> {
    await delay(300);
    console.log(`[Mock] Winnow rule set: ${path} -> ${moduleId}`);
  },
  openLink(url: string): void {
    console.log('[Mock] Opening link:', url);
    window.open(url, '_blank');
  },
  async reboot(): Promise<void> {
    console.log('[Mock] Rebooting...');
    await delay(1000);
    window.location.reload(); 
  }
};