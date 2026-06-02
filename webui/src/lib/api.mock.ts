import { APP_VERSION } from "./constants_gen";
import { DEFAULT_CONFIG } from "./constants";
import type { AppAPI } from "./api/contracts";
import type {
  AppConfig,
  KasumiLkmStatus,
  KasumiStatus,
  KasumiUnameConfig,
  KernelUnameValues,
  ModeStats,
  Module,
  ModuleRules,
  StorageStatus,
  SystemInfo,
} from "./types";

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const KASUMI_LKM_DIR = "/data/adb/modules/hybrid_mount/kasumi_lkm";
const KASUMI_CURRENT_KMI = "android16-6.12";
const KASUMI_LKM_FILE =
  "/data/adb/modules/hybrid_mount/kasumi_lkm/android16-6.12_arm64_kasumi_lkm.ko";

function createMockState() {
  return {
    version: APP_VERSION,
    mountErrorsCleared: false,
    kasumi: {
      enabled: true,
      lkmLoaded: true,
      ruleCount: 3,
      lkmAutoload: true,
      kmiOverride: "",
      mirrorPath: "/dev/kasumi_mirror",
      stealth: true,
      hideXattr: false,
      selinuxFix: false,
      kernelDebug: false,
      mapsSpoof: true,
      mountHideEnabled: false,
      mountHidePathPattern: "",
      statfsSpoofEnabled: false,
      statfsSpoofPath: "",
      statfsSpoofFtype: 0,
      cmdline: "androidboot.verifiedbootstate=green",
      unameMode: "scoped" as "scoped" | "global",
      uname: {
        sysname: "",
        nodename: "",
        release: "6.12.0-android16-gki",
        version: "#1 SMP PREEMPT",
        machine: "",
        domainname: "",
      },
      originalKernel: {
        release: "6.12.0-android16-gki",
        version: "#1 SMP PREEMPT Mon May 11 18:20:00 CST 2026",
      },
      hideUids: [1000],
      mapsRules: [
        {
          target_ino: 12345,
          target_dev: 2049,
          spoofed_ino: 54321,
          spoofed_dev: 2050,
          spoofed_pathname: "/system/bin/app_process64",
        },
      ],
      userHideRules: ["/data/adb/magisk"],
    },
  };
}

const mockState = createMockState();

function buildMockLkmStatus(): KasumiLkmStatus {
  const { kasumi } = mockState;
  return {
    loaded: kasumi.lkmLoaded,
    module_name: "kasumi_lkm",
    autoload: kasumi.lkmAutoload,
    kmi_override: kasumi.kmiOverride,
    current_kmi: KASUMI_CURRENT_KMI,
    search_dir: KASUMI_LKM_DIR,
    module_file: kasumi.lkmLoaded ? KASUMI_LKM_FILE : "",
    last_error: null,
  };
}

function buildMockKasumiConfig(enabled: boolean): KasumiStatus["config"] {
  const { kasumi } = mockState;
  return {
    enabled,
    lkm_autoload: kasumi.lkmAutoload,
    lkm_dir: KASUMI_LKM_DIR,
    lkm_kmi_override: kasumi.kmiOverride,
    mirror_path: kasumi.mirrorPath,
    enable_kernel_debug: kasumi.kernelDebug,
    enable_stealth: kasumi.stealth,
    enable_hidexattr: kasumi.hideXattr,
    enable_selinux_fix: kasumi.selinuxFix,
    enable_mount_hide: kasumi.mountHideEnabled,
    enable_maps_spoof: kasumi.mapsSpoof,
    enable_statfs_spoof: kasumi.statfsSpoofEnabled,
    mount_hide: {
      enabled: kasumi.mountHideEnabled,
      path_pattern: kasumi.mountHidePathPattern,
    },
    statfs_spoof: {
      enabled: kasumi.statfsSpoofEnabled,
      path: kasumi.statfsSpoofPath,
      spoof_f_type: kasumi.statfsSpoofFtype,
    },
    hide_uids: [...kasumi.hideUids],
    uname_mode: kasumi.unameMode,
    uname: { ...kasumi.uname },
    cmdline_value: kasumi.cmdline,
    kstat_rules: [],
    maps_rules: kasumi.mapsRules.map((rule) => ({ ...rule })),
  };
}

function buildMockKasumiStatus(): KasumiStatus {
  const { kasumi } = mockState;
  const lkm = buildMockLkmStatus();

  if (!kasumi.enabled) {
    return {
      status: "disabled",
      available: false,
      kernel_supported: false,
      protocol_version: null,
      feature_bits: null,
      feature_names: [],
      hooks: [],
      rule_count: 0,
      user_hide_rule_count: kasumi.userHideRules.length,
      mirror_path: kasumi.mirrorPath,
      lkm,
      config: buildMockKasumiConfig(false),
      runtime: {
        snapshot: { status: "disabled" },
        kasumi_modules: [],
      },
    };
  }

  const available = kasumi.lkmLoaded;
  return {
    status: available ? "available" : "unavailable",
    available,
    kernel_supported: true,
    protocol_version: available ? 16 : null,
    feature_bits: available ? 0x7f7 : null,
    feature_names: available
      ? [
          "kstat_spoof",
          "uname_spoof",
          "cmdline_spoof",
          "selinux_bypass",
          "merge_dir",
          "mount_hide",
          "maps_spoof",
          "statfs_spoof",
          "fake_mountinfo",
          "selinux_fix",
        ]
      : [],
    hooks: available ? ["d_path", "iterate_dir", "vfs_getattr"] : [],
    rule_count: available ? kasumi.ruleCount : 0,
    user_hide_rule_count: kasumi.userHideRules.length,
    mirror_path: kasumi.mirrorPath,
    lkm,
    config: buildMockKasumiConfig(true),
    runtime: {
      snapshot: {
        status: available ? "enabled" : "unavailable",
      },
      kasumi_modules: available ? ["playintegrityfix"] : [],
    },
  };
}

function buildMockSystemInfo(): SystemInfo {
  return {
    kernel: "Linux localhost 6.12.0-android16-gki #1 SMP PREEMPT",
    selinux: "Enforcing",
    mountBase: "/data/adb/hybrid-mount/mnt",
    activeMounts: ["system", "product"],
    tmpfs_xattr_supported: false,
    supported_overlay_modes: ["ext4"],
  };
}

function buildMockModules(): Module[] {
  return [
    {
      id: "magisk_module_1",
      name: "Example Module",
      version: "1.0.0",
      author: "Developer",
      description: "This is a mock module for testing.",
      mode: "magic",
      is_mounted: true,
      enabled: true,
      source_path: "/data/adb/modules/magisk_module_1",
      rules: {
        default_mode: "magic",
        paths: { "system/fonts": "overlay" },
      },
    },
    {
      id: "overlay_module_2",
      name: "System UI Overlay",
      version: "2.5",
      author: "Google",
      description: "Changes system colors.",
      mode: "overlay",
      is_mounted: true,
      enabled: true,
      source_path: "/data/adb/modules/overlay_module_2",
      rules: {
        default_mode: "overlay",
        paths: {},
      },
    },
    {
      id: "playintegrityfix",
      name: "Play Integrity Fix",
      version: "14.2",
      author: "chiteroman",
      description:
        "Mirror-backed Kasumi module for passing Play Integrity checks.",
      mode: "kasumi",
      is_mounted: true,
      enabled: true,
      source_path: "/data/adb/modules/playintegrityfix",
      rules: {
        default_mode: "kasumi",
        paths: {},
      },
    },
    {
      id: "disabled_module",
      name: "Umount Module",
      version: "0.1",
      author: "Tester",
      description: "This module has a mount error.",
      mode: "ignore",
      is_mounted: false,
      enabled: true,
      source_path: "/data/adb/modules/disabled_module",
      mount_error: mockState.mountErrorsCleared
        ? undefined
        : "stage=execute; error=mock mount failure",
      suggest_ignore: mockState.mountErrorsCleared ? undefined : true,
      rules: {
        default_mode: "ignore",
        paths: {},
      },
    },
    {
      id: "blacklisted_example",
      name: "Blacklisted Module",
      version: "0.5",
      author: "Unknown",
      description: "This module is blacklisted and skipped during mount.",
      mode: "ignore",
      is_mounted: false,
      enabled: false,
      source_path: "/data/adb/modules/blacklisted_example",
      rules: {
        default_mode: "ignore",
        paths: {},
      },
    },
  ];
}

function buildModeStats(): ModeStats {
  return {
    overlay: 1,
    magic: 1,
    kasumi: 1,
    blacklisted: 1,
  };
}

export const MockAPI: AppAPI = {
  async wakeDaemon(): Promise<void> {
    await delay(20);
  },

  async init() {
    await delay(200);
    return {
      status: {
        storage_mode: "tmpfs",
        mount_point: "/data/adb/hybrid-mount/mnt",
        overlay_modules: ["overlay_module_2"],
        magic_modules: ["magisk_module_1"],
        kasumi_modules: ["playintegrityfix"],
        mount_error_modules: mockState.mountErrorsCleared
          ? []
          : ["disabled_module"],
        blacklisted_modules: ["blacklisted_example"],
        active_mounts: ["system", "product"],
        tmpfs_xattr_supported: false,
        mode_stats: {
          overlayfs: 1,
          magicmount: 1,
          kasumi: 1,
          blacklisted: 1,
        },
      },
      config: { ...DEFAULT_CONFIG },
      version: mockState.version,
      kasumi_status: buildMockKasumiStatus(),
      system_info: buildMockSystemInfo(),
    };
  },

  async loadConfig(): Promise<AppConfig> {
    await delay(300);
    return { ...DEFAULT_CONFIG };
  },

  async saveConfig(config: AppConfig): Promise<void> {
    await delay(500);
    console.log("[Mock] Config saved:", config);
  },

  async resetConfig(): Promise<void> {
    await delay(500);
    console.log("[Mock] Config reset to defaults");
  },

  async scanModules(_dir?: string): Promise<Module[]> {
    await delay(600);
    return buildMockModules();
  },

  async saveModules(modules: Module[]): Promise<void> {
    await delay(400);
    console.log("[Mock] Modules saved:", modules);
  },

  async saveModuleRules(moduleId: string, rules: ModuleRules): Promise<void> {
    await delay(400);
    console.log(`[Mock] Rules saved for ${moduleId}:`, rules);
  },

  async saveAllModuleRules(rules: Record<string, ModuleRules>): Promise<void> {
    await delay(400);
    console.log("[Mock] All module rules saved:", rules);
  },

  async getVersion(): Promise<string> {
    await delay(100);
    return mockState.version;
  },

  async getStorageUsage(): Promise<StorageStatus> {
    await delay(300);
    return {
      type: "ext4",
      supported_modes: ["tmpfs", "ext4"],
      modeStats: buildModeStats(),
      mountedCount: 3,
    };
  },

  async getSystemInfo(): Promise<SystemInfo> {
    await delay(300);
    return buildMockSystemInfo();
  },

  async getKasumiStatus(): Promise<KasumiStatus> {
    await delay(300);
    return buildMockKasumiStatus();
  },

  async setKasumiEnabled(enabled: boolean): Promise<void> {
    await delay(200);
    mockState.kasumi.enabled = enabled;
  },

  async setKasumiStealth(enabled: boolean): Promise<void> {
    await delay(200);
    mockState.kasumi.stealth = enabled;
  },

  async setKasumiHidexattr(enabled: boolean): Promise<void> {
    await delay(200);
    mockState.kasumi.hideXattr = enabled;
  },

  async setKasumiSelinuxFix(enabled: boolean): Promise<void> {
    await delay(200);
    mockState.kasumi.selinuxFix = enabled;
  },

  async setKasumiDebug(enabled: boolean): Promise<void> {
    await delay(200);
    mockState.kasumi.kernelDebug = enabled;
  },

  async getOriginalKernelUname(): Promise<KernelUnameValues> {
    await delay(120);
    return { ...mockState.kasumi.originalKernel };
  },

  async setKasumiUnameMode(mode: "scoped" | "global"): Promise<void> {
    await delay(120);
    mockState.kasumi.unameMode = mode;
  },

  async setKasumiUname(uname: Partial<KasumiUnameConfig>): Promise<void> {
    await delay(220);
    mockState.kasumi.uname = {
      ...mockState.kasumi.uname,
      ...uname,
    };
  },

  async applyKasumiUname(
    mode: "scoped" | "global",
    uname: Pick<KasumiUnameConfig, "release" | "version">,
  ): Promise<void> {
    await delay(220);
    mockState.kasumi.unameMode = mode;
    mockState.kasumi.uname.release = uname.release;
    mockState.kasumi.uname.version = uname.version;
  },

  async clearKasumiUname(mode: "scoped" | "global" = "scoped"): Promise<void> {
    await delay(160);
    mockState.kasumi.unameMode = mode;
    mockState.kasumi.uname = {
      sysname: "",
      nodename: "",
      release: "",
      version: "",
      machine: "",
      domainname: "",
    };
  },

  async restoreKasumiUnameGlobal(): Promise<void> {
    await delay(160);
    mockState.kasumi.unameMode = "global";
    mockState.kasumi.uname = {
      sysname: "",
      nodename: "",
      release: "",
      version: "",
      machine: "",
      domainname: "",
    };
  },

  async setKasumiCmdline(value: string): Promise<void> {
    await delay(220);
    mockState.kasumi.cmdline = value;
  },

  async clearKasumiCmdline(): Promise<void> {
    await delay(160);
    mockState.kasumi.cmdline = "";
  },

  async addKasumiMapsRule(rule): Promise<void> {
    await delay(180);
    const nextRule = {
      target_ino: Number(rule.target_ino) || 0,
      target_dev: Number(rule.target_dev) || 0,
      spoofed_ino: Number(rule.spoofed_ino) || 0,
      spoofed_dev: Number(rule.spoofed_dev) || 0,
      spoofed_pathname: rule.spoofed_pathname || "",
    };
    mockState.kasumi.mapsRules = mockState.kasumi.mapsRules.filter(
      (item) =>
        !(
          item.target_ino === nextRule.target_ino &&
          item.target_dev === nextRule.target_dev
        ),
    );
    mockState.kasumi.mapsRules.push(nextRule);
  },

  async clearMountErrors(): Promise<void> {
    await delay(180);
    mockState.mountErrorsCleared = true;
  },

  async clearKasumiMapsRules(): Promise<void> {
    await delay(180);
    mockState.kasumi.mapsRules = [];
  },

  async getUserHideRules(): Promise<string[]> {
    await delay(120);
    return [...mockState.kasumi.userHideRules];
  },

  async addUserHideRule(path: string): Promise<void> {
    await delay(180);
    if (!mockState.kasumi.userHideRules.includes(path)) {
      mockState.kasumi.userHideRules = [
        path,
        ...mockState.kasumi.userHideRules,
      ];
    }
  },

  async removeUserHideRule(path: string): Promise<void> {
    await delay(180);
    mockState.kasumi.userHideRules = mockState.kasumi.userHideRules.filter(
      (value) => value !== path,
    );
  },

  async applyUserHideRules(): Promise<void> {
    await delay(180);
  },

  async loadKasumiLkm(): Promise<void> {
    await delay(260);
    mockState.kasumi.lkmLoaded = true;
  },

  async unloadKasumiLkm(): Promise<void> {
    await delay(260);
    mockState.kasumi.lkmLoaded = false;
  },

  async setKasumiLkmAutoload(enabled: boolean): Promise<void> {
    await delay(160);
    mockState.kasumi.lkmAutoload = enabled;
  },

  async setKasumiLkmKmi(value: string): Promise<void> {
    await delay(160);
    mockState.kasumi.kmiOverride = value;
  },

  async clearKasumiLkmKmi(): Promise<void> {
    await delay(160);
    mockState.kasumi.kmiOverride = "";
  },

  async fixKasumiMounts(): Promise<void> {
    await delay(180);
  },

  async clearKasumiRules(): Promise<void> {
    await delay(180);
    mockState.kasumi.ruleCount = 0;
  },

  async releaseKasumiConnection(): Promise<void> {
    await delay(120);
  },

  async invalidateKasumiCache(): Promise<void> {
    await delay(120);
  },

  async openLink(url: string): Promise<void> {
    await delay(100);
    window.open(url, "_blank", "noopener,noreferrer");
  },

  async reboot(): Promise<void> {
    await delay(120);
  },
};
