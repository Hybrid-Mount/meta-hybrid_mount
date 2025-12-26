/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { API } from './api';
import { DEFAULT_CONFIG, DEFAULT_SEED } from './constants';
import { APP_VERSION } from './constants_gen';
import { Monet } from './theme';
import type { 
  AppConfig, 
  Module, 
  StorageStatus, 
  SystemInfo, 
  DeviceInfo, 
  ToastMessage, 
  LanguageOption,
  ModeStats,
  ConflictEntry,
  DiagnosticIssue
} from './types';

const localeModules = import.meta.glob('../locales/*.json', { eager: true });

export interface LogEntry {
  text: string;
  type: 'info' | 'warn' | 'error' | 'debug';
}

const createStore = () => {
  let theme = $state<'auto' | 'light' | 'dark'>('auto');
  let isSystemDark = $state(false);
  let lang = $state('en');
  let seed = $state<string | null>(DEFAULT_SEED);
  let loadedLocale = $state<any>(null);
  let toast = $state<ToastMessage>({ id: 'init', text: '', type: 'info', visible: false });
  let fixBottomNav = $state(false);

  const availableLanguages: LanguageOption[] = Object.entries(localeModules).map(([path, mod]: [string, any]) => {
    const match = path.match(/\/([^/]+)\.json$/);
    const code = match ? match[1] : 'en';
    const name = mod.default?.lang?.display || code.toUpperCase();
    return { code, name };
  }).sort((a, b) => {
    if (a.code === 'en') return -1;
    if (b.code === 'en') return 1;
    return a.name.localeCompare(b.name);
  });

  let config = $state<AppConfig>(DEFAULT_CONFIG);
  let modules = $state<Module[]>([]);
  let logs = $state<LogEntry[]>([]);
  let device = $state<DeviceInfo>({ model: '-', android: '-', kernel: '-', selinux: '-' });
  let version = $state(APP_VERSION);
  let storage = $state<StorageStatus>({ 
    used: '-', 
    size: '-', 
    percent: '0%', 
    type: null,
    hymofs_available: false 
  });
  let systemInfo = $state<SystemInfo>({ kernel: '-', selinux: '-', mountBase: '-', activeMounts: [] });
  let activePartitions = $state<string[]>([]);
  let conflicts = $state<ConflictEntry[]>([]);
  let diagnostics = $state<DiagnosticIssue[]>([]);
  
  let loadingConfig = $state(false);
  let loadingModules = $state(false);
  let loadingLogs = $state(false);
  let loadingStatus = $state(false);
  let loadingConflicts = $state(false);
  let loadingDiagnostics = $state(false);
  let savingConfig = $state(false);
  let savingModules = $state(false);

  let L = $derived(loadedLocale?.default || {});

  let modeStats = $derived.by((): ModeStats => {
    const stats = { auto: 0, magic: 0, hymofs: 0 };
    modules.forEach(m => {
        if (!m.is_mounted) return;
        if (m.mode === 'auto') stats.auto++;
        else if (m.mode === 'magic') stats.magic++;
        else if (m.mode === 'hymofs') stats.hymofs++;
    });
    return stats;
  });

  function showToast(text: string, type: 'info' | 'success' | 'error' = 'info') {
    const id = Date.now().toString();
    toast = { id, text, type, visible: true };
    setTimeout(() => {
      if (toast.id === id) {
        toast.visible = false;
      }
    }, 3000);
  }

  function setTheme(t: 'auto' | 'light' | 'dark') {
    theme = t;
    applyTheme();
  }

  function applyTheme() {
    const isDark = theme === 'auto' ? isSystemDark : theme === 'dark';
    document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
    Monet.apply(seed, isDark);
  }

  async function loadLocale(code: string) {
    const match = Object.entries(localeModules).find(([path]) => path.endsWith(`/${code}.json`));
    if (match) {
        loadedLocale = match[1];
    } else {
        loadedLocale = localeModules['../locales/en.json'];
    }
  }

  function setLang(code: string) {
    lang = code;
    localStorage.setItem('lang', code);
    loadLocale(code);
  }

  function toggleBottomNavFix() {
    fixBottomNav = !fixBottomNav;
    localStorage.setItem('hm_fix_bottom_nav', String(fixBottomNav));
    const msg = fixBottomNav 
        ? (L.config?.fixBottomNavOn || 'Bottom Nav Fix Enabled') 
        : (L.config?.fixBottomNavOff || 'Bottom Nav Fix Disabled');
    showToast(msg, 'info');
  }

  async function init() {
    const savedLang = localStorage.getItem('lang') || 'en';
    lang = savedLang;
    await loadLocale(savedLang);

    fixBottomNav = localStorage.getItem('hm_fix_bottom_nav') === 'true';

    const darkModeQuery = window.matchMedia('(prefers-color-scheme: dark)');
    isSystemDark = darkModeQuery.matches;
    darkModeQuery.addEventListener('change', (e) => {
      isSystemDark = e.matches;
      applyTheme();
    });

    try {
        const sysColor = await API.fetchSystemColor();
        if (sysColor) {
            seed = sysColor;
        }
    } catch {}
    applyTheme();

    await Promise.all([
      loadConfig(),
      loadStatus()
    ]);
  }

  async function loadConfig() {
    loadingConfig = true;
    try {
      config = await API.loadConfig();
    } catch (e) {
      showToast('Failed to load config', 'error');
    }
    loadingConfig = false;
  }

  async function saveConfig() {
    savingConfig = true;
    try {
      await API.saveConfig($state.snapshot(config));
      showToast(L.common?.saved || 'Saved', 'success');
    } catch (e) {
      showToast('Failed to save config', 'error');
    }
    savingConfig = false;
  }

  async function resetConfig() {
    savingConfig = true;
    try {
      await API.resetConfig();
      await loadConfig();
      showToast(L.config?.resetSuccess || 'Config reset to defaults', 'success');
    } catch (e) {
      showToast('Failed to reset config', 'error');
    }
    savingConfig = false;
  }

  async function loadModules() {
    loadingModules = true;
    try {
      modules = await API.scanModules(config.moduledir);
    } catch (e) {
      showToast('Failed to load modules', 'error');
    }
    loadingModules = false;
  }

  async function saveModules() {
    savingModules = true;
    try {
      await API.saveModules($state.snapshot(modules));
      showToast(L.common?.saved || 'Saved', 'success');
    } catch (e) {
      showToast('Failed to save module modes', 'error');
    }
    savingModules = false;
  }

  async function loadLogs(silent: boolean = false) {
    if (!silent) loadingLogs = true;
    try {
      const rawLogs = await API.readLogs();
      logs = rawLogs.split('\n').map(line => {
        const text = line.replace(/^[\d-]{10}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?\s*/, '');
        let type: LogEntry['type'] = 'info';
        if (text.includes('[E]') || text.includes('[ERROR]')) type = 'error';
        else if (text.includes('[W]') || text.includes('[WARN]')) type = 'warn';
        else if (text.includes('[D]') || text.includes('[DEBUG]')) type = 'debug';
        return { text, type };
      });
    } catch (e) {
      logs = [{ text: "Failed to load logs.", type: 'error' }];
    }
    loadingLogs = false;
  }

  async function loadStatus() {
    loadingStatus = true;
    try {
      device = await API.getDeviceStatus();
      version = await API.getVersion();
      storage = await API.getStorageUsage();
      systemInfo = await API.getSystemInfo();
      activePartitions = systemInfo.activeMounts || [];
      if (modules.length === 0) {
        await loadModules();
      }
      
      loadingDiagnostics = true;
      diagnostics = await API.getDiagnostics();
      loadingDiagnostics = false;

    } catch (e) {}
    loadingStatus = false;
  }

  async function loadConflicts() {
      loadingConflicts = true;
      try {
          conflicts = await API.getConflicts();
          if (conflicts.length === 0) {
              showToast(L.modules?.noConflicts || "No conflicts detected", "success");
          }
      } catch (e) {
          showToast(L.modules?.conflictError || "Failed to check conflicts", "error");
      }
      loadingConflicts = false;
  }

  return {
    get theme() { return theme; },
    get isSystemDark() { return isSystemDark; },
    get lang() { return lang; },
    get seed() { return seed; },
    get availableLanguages() { return availableLanguages; },
    get L() { return L; },
    get toast() { return toast; },
    get toasts() { return toast.visible ? [toast] : []; },
    get fixBottomNav() { return fixBottomNav; },
    toggleBottomNavFix,
    showToast,
    setTheme,
    setLang,
    init,
    get config() { return config; },
    set config(v) { config = v; },
    loadConfig,
    saveConfig,
    resetConfig,
    get modules() { return modules; },
    set modules(v) { modules = v; },
    get modeStats() { return modeStats; },
    loadModules,
    saveModules,
    get logs() { return logs; },
    loadLogs,
    get device() { return device; },
    get version() { return version; },
    get storage() { return storage; },
    get systemInfo() { return systemInfo; },
    get activePartitions() { return activePartitions; },
    get conflicts() { return conflicts; },
    loadConflicts,
    get diagnostics() { return diagnostics; },
    loadStatus,
    get loading() {
      return {
        config: loadingConfig,
        modules: loadingModules,
        logs: loadingLogs,
        status: loadingStatus,
        conflicts: loadingConflicts,
        diagnostics: loadingDiagnostics
      };
    },
    get saving() {
      return {
        config: savingConfig,
        modules: savingModules
      };
    }
  };
};

export const store = createStore();