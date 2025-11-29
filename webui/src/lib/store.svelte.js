import { API } from './api';
import { DEFAULT_CONFIG, DEFAULT_SEED } from './constants';
import { Monet } from './theme';

const localeModules = import.meta.glob('../locales/*.json', { eager: true });

// Global state using Svelte 5 Runes
export const store = $state({
  config: { ...DEFAULT_CONFIG },
  modules: [],
  logs: [],
  storage: { used: '-', size: '-', percent: '0%' },
  systemInfo: { kernel: '-', selinux: '-', mountBase: '-' },
  activePartitions: [], // List of currently mounted partitions
  
  // UI State
  loading: { config: false, modules: false, logs: false, status: false },
  saving: { config: false, modules: false },
  toast: { text: '', type: 'info', visible: false },
  
  // Settings
  theme: 'dark',
  lang: 'en',
  seed: DEFAULT_SEED,
  loadedLocale: null, // Stores the content of the loaded JSON

  // Computed: Available languages list for UI
  get availableLanguages() {
    return Object.entries(localeModules).map(([path, mod]) => {
      const match = path.match(/\/([^/]+)\.json$/);
      const code = match ? match[1] : 'en';
      const name = mod.default?.lang?.display || code.toUpperCase();
      return { code, name };
    }).sort((a, b) => {
      if (a.code === 'en') return -1;
      if (b.code === 'en') return 1;
      return a.code.localeCompare(b.code);
    });
  },

  // Getters
  get L() {
    return this.loadedLocale || this.getFallbackLocale();
  },

  // Helper for initial safe state
  getFallbackLocale() {
    return {
        common: { appName: "Hybrid Mount", saving: "...", theme: "Theme", language: "Language" },
        lang: { display: "English" },
        tabs: { status: "Status", config: "Config", modules: "Modules", logs: "Logs" },
        status: { storageTitle: "Storage", storageDesc: "", moduleTitle: "Modules", moduleActive: "Active", modeStats: "Stats", modeAuto: "Auto", modeMagic: "Magic", sysInfoTitle: "System Info", kernel: "Kernel", selinux: "SELinux", mountBase: "Mount Base", activePartitions: "Active Partitions" },
        config: { title: "Config", verboseLabel: "Verbose", verboseOff: "Off", verboseOn: "On", forceExt4: "Force Ext4", enableNuke: "Nuke LKM", disableUmount: "Disable Umount", moduleDir: "Dir", tempDir: "Temp", mountSource: "Source", logFile: "Log", partitions: "Partitions", autoPlaceholder: "Auto", reload: "Reload", save: "Save", reset: "Reset to Auto", invalidPath: "Invalid path detected", loadSuccess: "", loadError: "", loadDefault: "", saveSuccess: "", saveFailed: "" },
        modules: { title: "Modules", desc: "", modeAuto: "Overlay", modeMagic: "Magic", scanning: "...", reload: "Refresh", save: "Save", empty: "Empty", scanError: "", saveSuccess: "", saveFailed: "", searchPlaceholder: "Search", filterLabel: "Filter", filterAll: "All" },
        logs: { title: "Logs", loading: "...", refresh: "Refresh", empty: "Empty", copy: "Copy", copySuccess: "Copied", copyFail: "Failed", searchPlaceholder: "Search", filterLabel: "Filter", levels: { all: "All", info: "Info", warn: "Warn", error: "Error" } }
    };
  },

  get modeStats() {
    let auto = 0;
    let magic = 0;
    this.modules.forEach(m => {
      if (m.mode === 'magic') magic++;
      else auto++;
    });
    return { auto, magic };
  },

  // Actions
  showToast(msg, type = 'info') {
    this.toast = { text: msg, type, visible: true };
    setTimeout(() => { this.toast.visible = false; }, 3000);
  },

  setTheme(newTheme) {
    this.theme = newTheme;
    document.documentElement.setAttribute('data-theme', newTheme);
    localStorage.setItem('mm-theme', newTheme);
    Monet.apply(this.seed, newTheme === 'dark');
  },

  async setLang(code) {
    const path = `../locales/${code}.json`;
    if (localeModules[path]) {
      try {
        // Since we loaded eagerly, we can just access .default
        const mod = localeModules[path];
        this.loadedLocale = mod.default; 
        this.lang = code;
        localStorage.setItem('mm-lang', code);
      } catch (e) {
        console.error(`Failed to load locale: ${code}`, e);
        if (code !== 'en') await this.setLang('en');
      }
    }
  },

  async init() {
    const savedLang = localStorage.getItem('mm-lang') || 'en';
    await this.setLang(savedLang);
    
    const savedTheme = localStorage.getItem('mm-theme');
    const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    this.setTheme(savedTheme || (systemDark ? 'dark' : 'light'));

    const sysColor = await API.fetchSystemColor();
    if (sysColor) {
      this.seed = sysColor;
      Monet.apply(this.seed, this.theme === 'dark');
    }

    await this.loadConfig();
  },

  async loadConfig() {
    this.loading.config = true;
    try {
      this.config = await API.loadConfig();
      if (this.L && this.L.config) {
          this.showToast(this.L.config.loadSuccess);
      }
    } catch (e) {
      if (this.L && this.L.config) {
          this.showToast(this.L.config.loadError, 'error');
      }
    }
    this.loading.config = false;
  },

  async saveConfig() {
    this.saving.config = true;
    try {
      await API.saveConfig(this.config);
      this.showToast(this.L.config.saveSuccess);
    } catch (e) {
      this.showToast(this.L.config.saveFailed, 'error');
    }
    this.saving.config = false;
  },

  async loadModules() {
    this.loading.modules = true;
    this.modules = [];
    try {
      this.modules = await API.scanModules(this.config.moduledir);
    } catch (e) {
      this.showToast(this.L.modules.scanError, 'error');
    }
    this.loading.modules = false;
  },

  async saveModules() {
    this.saving.modules = true;
    try {
      await API.saveModules(this.modules);
      this.showToast(this.L.modules.saveSuccess);
    } catch (e) {
      this.showToast(this.L.modules.saveFailed, 'error');
    }
    this.saving.modules = false;
  },

  async loadLogs(silent = false) {
    if (!silent) this.loading.logs = true;
    if (!silent) this.logs = []; 
    
    try {
      const raw = await API.readLogs(this.config.logfile, 1000);
      
      if (!raw) {
        this.logs = [{ text: this.L.logs.empty, type: 'debug' }];
      } else {
        this.logs = raw.split('\n').map(line => {
          let type = 'debug';
          if (line.includes('[ERROR]')) type = 'error';
          else if (line.includes('[WARN]')) type = 'warn';
          else if (line.includes('[INFO]')) type = 'info';
          return { text: line, type };
        });
      }
    } catch (e) {
      console.error(e);
      this.logs = [{ text: `Error: ${e.message}`, type: 'error' }];
      if (!silent) this.showToast(this.L.logs.readFailed, 'error');
    }
    this.loading.logs = false;
  },

  async loadStatus() {
    this.loading.status = true;
    try {
      // Parallel fetch: Storage, System Info, Active Mounts
      const [storageData, sysInfoData, activeMounts] = await Promise.all([
        API.getStorageUsage(),
        API.getSystemInfo(),
        API.getActiveMounts(this.config.mountsource)
      ]);
      
      this.storage = storageData;
      this.systemInfo = sysInfoData;
      this.activePartitions = activeMounts;

      if (this.modules.length === 0) {
        this.modules = await API.scanModules(this.config.moduledir);
      }
    } catch (e) {
      // ignore
    }
    this.loading.status = false;
  }
});