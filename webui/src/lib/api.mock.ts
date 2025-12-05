import { DEFAULT_CONFIG } from './constants';
import type { AppConfig, Module, StorageStatus, SystemInfo, DeviceInfo } from './types';

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

interface MockStateType {
    modules: Module[];
    config: AppConfig;
    logs: string[];
}

const MOCK_STATE: MockStateType = {
    modules: [
        { id: "magisk_module_test", name: "Magisk Module", version: "1.0", author: "User", description: "A test module", mode: "auto", enabled: true },
        { id: "youtube_revanced", name: "YouTube ReVanced", version: "18.0.0", author: "ReVanced", description: "Extended YouTube", mode: "magic", enabled: true },
        { id: "my_custom_mod", name: "Custom Tweak", version: "v2", author: "Me", description: "System tweaks", mode: "auto", enabled: true }
    ],
    config: { ...DEFAULT_CONFIG, partitions: ["product", "system_ext"] },
    logs: [
        "[INFO] Meta-Hybrid Daemon v0.2.8 started",
        "[INFO] Storage backend: tmpfs (XATTR supported)",
        "[INFO] Mounting overlay for /system...",
        "[WARN] /vendor overlay skipped: target busy",
        "[INFO] Magic mount active: youtube_revanced",
        "[INFO] System operational."
    ]
};

export const MockAPI = {
    loadConfig: async (): Promise<AppConfig> => {
        await delay(500);
        return MOCK_STATE.config;
    },

    saveConfig: async (config: AppConfig): Promise<void> => {
        await delay(800);
        MOCK_STATE.config = config;
        console.log("[Mock] Config Saved:", config);
    },

    scanModules: async (): Promise<Module[]> => {
        await delay(1000);
        return MOCK_STATE.modules;
    },

    saveModules: async (modules: Module[]): Promise<void> => {
        await delay(600);
        MOCK_STATE.modules = modules;
        console.log("[Mock] Module Modes Saved:", modules.map(m => `${m.id}=${m.mode}`));
    },

    readLogs: async (): Promise<string> => {
        await delay(400);
        return MOCK_STATE.logs.join('\n');
    },

    getStorageUsage: async (): Promise<StorageStatus> => {
        await delay(600);
        return {
            size: '3.8G',
            used: '1.2G',
            percent: '31%',
            type: 'tmpfs' 
        };
    },

    getSystemInfo: async (): Promise<SystemInfo> => {
        await delay(600);
        return {
            kernel: '5.10.177-android12-9-00001-g5d3f2a (Mock)',
            selinux: 'Enforcing',
            mountBase: '/data/adb/meta-hybrid/mnt',
            activeMounts: ['system', 'product', 'system_ext']
        };
    },

    getDeviceStatus: async (): Promise<DeviceInfo> => {
        await delay(500);
        return {
            model: 'Pixel 7 Pro (Mock)',
            android: '14',
            kernel: '5.10.177-android12-9-00001-g5d3f2a',
            selinux: 'Enforcing'
        };
    },

    getVersion: async (): Promise<string> => {
        await delay(300);
        return "v1.0.1-r1-mock";
    },

    getActiveMounts: async (): Promise<string[]> => {
        return ['system', 'product'];
    },

    openLink: async (url: string): Promise<void> => {
        console.log("[Mock] Opening URL:", url);
        window.open(url, '_blank');
    },

    fetchSystemColor: async (): Promise<string> => {
        await delay(200);
        return '#8FBC8F'; 
    }
};