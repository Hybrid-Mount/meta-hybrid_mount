import { createSignal, createMemo, createRoot } from "solid-js";
import { API } from "./api";
import { DEFAULT_CONFIG } from "./constants";
import { APP_VERSION } from "./constants_gen";
import type {
  AppConfig,
  Module,
  StorageStatus,
  SystemInfo,
  DeviceInfo,
  ToastMessage,
  LanguageOption,
  ModeStats,
} from "./types";

const localeModules = import.meta.glob("../locales/*.json", { eager: true });

type LocaleDict = any;

export interface LogEntry {
  text: string;
  type: "info" | "warn" | "error" | "debug";
}

const createGlobalStore = () => {
  const [lang, setLangSignal] = createSignal("en-US");
  const [loadedLocale, setLoadedLocale] = createSignal<unknown>(null);

  const [toast, setToast] = createSignal<ToastMessage>({
    id: "init",
    text: "",
    type: "info",
    visible: false,
  });

  const [fixBottomNav, setFixBottomNavSignal] = createSignal(false);

  const [config, setConfig] = createSignal<AppConfig>(DEFAULT_CONFIG);
  const [modules, setModules] = createSignal<Module[]>([]);
  const [device, setDevice] = createSignal<DeviceInfo>({
    model: "-",
    android: "-",
    kernel: "-",
    selinux: "-",
  });
  const [version, setVersion] = createSignal(APP_VERSION);
  const [storage, setStorage] = createSignal<StorageStatus>({
    type: null,
  });
  const [systemInfo, setSystemInfo] = createSignal<SystemInfo>({
    kernel: "-",
    selinux: "-",
    mountBase: "-",
    activeMounts: [],
  });
  const [activePartitions, setActivePartitions] = createSignal<string[]>([]);

  const [loadingConfig, setLoadingConfig] = createSignal(false);
  const [loadingModules, setLoadingModules] = createSignal(false);
  const [loadingStatus, setLoadingStatus] = createSignal(false);

  const [savingConfig, setSavingConfig] = createSignal(false);
  const [savingModules, setSavingModules] = createSignal(false);

  const availableLanguages: LanguageOption[] = Object.entries(localeModules)
    .map(([path, mod]: [string, unknown]) => {
      const match = path.match(/\/([^/]+)\.json$/);
      const code = match ? match[1] : "en-US";
      const name =
        (mod as { default?: { lang?: { display?: string } } }).default?.lang
          ?.display || code.toUpperCase();
      return { code, name };
    })
    .sort((a, b) => {
      if (a.code === "en-US") return -1;
      if (b.code === "en-US") return 1;
      return a.name.localeCompare(b.name);
    });

  const L = createMemo(
    (): LocaleDict =>
      (loadedLocale() as { default: LocaleDict })?.default || {},
  );

  const modeStats = createMemo((): ModeStats => {
    const stats = { auto: 0, magic: 0 };
    modules().forEach((m) => {
      if (!m.is_mounted) return;
      if (m.mode === "auto") stats.auto++;
      else if (m.mode === "magic") stats.magic++;
    });
    return stats;
  });

  function showToast(
    text: string,
    type: "info" | "success" | "error" = "info",
  ) {
    const id = Date.now().toString();
    const newToast = { id, text, type, visible: true };
    setToast(newToast);
    setTimeout(() => {
      if (toast().id === id) {
        setToast((t) => ({ ...t, visible: false }));
      }
    }, 3000);
  }

  async function loadLocale(code: string) {
    const match = Object.entries(localeModules).find(([path]) =>
      path.endsWith(`/${code}.json`),
    );
    if (match) {
      setLoadedLocale(match[1]);
    } else {
      setLoadedLocale(localeModules["../locales/en-US.json"]);
    }
  }

  function setLang(code: string) {
    setLangSignal(code);
    localStorage.setItem("lang", code);
    loadLocale(code);
  }

  function toggleBottomNavFix() {
    const newVal = !fixBottomNav();
    setFixBottomNavSignal(newVal);
    localStorage.setItem("hm_fix_bottom_nav", String(newVal));

    const dict = L();
    const msg = newVal
      ? dict.config?.fixBottomNavOn || "Bottom Nav Fix Enabled"
      : dict.config?.fixBottomNavOff || "Bottom Nav Fix Disabled";
    showToast(msg, "info");
  }

  async function init() {
    const savedLang = localStorage.getItem("lang") || "en-US";
    setLangSignal(savedLang);
    await loadLocale(savedLang);

    setFixBottomNavSignal(localStorage.getItem("hm_fix_bottom_nav") === "true");

    await Promise.all([loadConfig(), loadStatus()]);
  }

  async function loadConfig() {
    setLoadingConfig(true);
    try {
      const data = await API.loadConfig();
      setConfig(data);
    } catch (e) {
      showToast(L().config?.loadError || "Failed to load config", "error");
    }
    setLoadingConfig(false);
  }

  async function saveConfig() {
    setSavingConfig(true);
    try {
      await API.saveConfig(config());
      showToast(L().common?.saved || "Saved", "success");
    } catch (e) {
      showToast(L().config?.saveFailed || "Failed to save config", "error");
    }
    setSavingConfig(false);
  }

  async function resetConfig() {
    setSavingConfig(true);
    try {
      await API.resetConfig();
      await loadConfig();
      showToast(
        L().config?.resetSuccess || "Config reset to defaults",
        "success",
      );
    } catch (e) {
      showToast(L().config?.saveFailed || "Failed to reset config", "error");
    }
    setSavingConfig(false);
  }

  async function loadModules() {
    setLoadingModules(true);
    try {
      const data = await API.scanModules(config().moduledir);
      setModules(data);
    } catch (e) {
      showToast(L().modules?.scanError || "Failed to load modules", "error");
    }
    setLoadingModules(false);
  }

  async function saveModules() {
    setSavingModules(true);
    try {
      await API.saveModules(modules());
      showToast(L().common?.saved || "Saved", "success");
    } catch (e) {
      showToast(
        L().modules?.saveFailed || "Failed to save module modes",
        "error",
      );
    }
    setSavingModules(false);
  }

  async function loadStatus() {
    setLoadingStatus(true);
    try {
      const d = await API.getDeviceStatus();
      setDevice(d);

      const v = await API.getVersion();
      setVersion(v);

      const s = await API.getStorageUsage();
      setStorage(s);

      const info = await API.getSystemInfo();
      setSystemInfo(info);
      setActivePartitions(info.activeMounts || []);

      if (modules().length === 0) {
        await loadModules();
      }
    } catch {}
    setLoadingStatus(false);
  }

  return {
    get lang() {
      return lang();
    },
    get availableLanguages() {
      return availableLanguages;
    },
    get L() {
      return L();
    },

    get toast() {
      return toast();
    },
    get toasts() {
      return toast().visible ? [toast()] : [];
    },

    get fixBottomNav() {
      return fixBottomNav();
    },
    toggleBottomNavFix,
    showToast,
    setLang,
    init,

    get config() {
      return config();
    },
    set config(v) {
      setConfig(v);
    },
    loadConfig,
    saveConfig,
    resetConfig,

    get modules() {
      return modules();
    },
    set modules(v) {
      setModules(v);
    },
    get modeStats() {
      return modeStats();
    },
    loadModules,
    saveModules,

    get device() {
      return device();
    },
    get version() {
      return version();
    },
    get storage() {
      return storage();
    },
    get systemInfo() {
      return systemInfo();
    },
    get activePartitions() {
      return activePartitions();
    },

    loadStatus,

    get loading() {
      return {
        get config() {
          return loadingConfig();
        },
        get modules() {
          return loadingModules();
        },
        get status() {
          return loadingStatus();
        },
      };
    },

    get saving() {
      return {
        get config() {
          return savingConfig();
        },
        get modules() {
          return savingModules();
        },
      };
    },
  };
};

export const store = createRoot(createGlobalStore);
