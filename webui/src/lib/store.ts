/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createMemo, createEffect } from 'solid-js';
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

const createGlobalStore = () => {
  const [theme, setThemeSignal] = createSignal<'auto' | 'light' | 'dark'>('auto');
  const [isSystemDark, setIsSystemDark] = createSignal(false);
  const [lang, setLangSignal] = createSignal('en');
  const [seed, setSeed] = createSignal<string | null>(DEFAULT_SEED);
  const [loadedLocale, setLoadedLocale] = createSignal<any>(null);
  
  const [toast, setToast] = createSignal<ToastMessage>({ id: 'init', text: '', type: 'info', visible: false });
  
  const [fixBottomNav, setFixBottomNavSignal] = createSignal(false);

  const [config, setConfig] = createSignal<AppConfig>(DEFAULT_CONFIG);
  const [modules, setModules] = createSignal<Module[]>([]);
  const [logs, setLogs] = createSignal<LogEntry[]>([]);
  const [device, setDevice] = createSignal<DeviceInfo>({ model: '-', android: '-', kernel: '-', selinux: '-' });
  const [version, setVersion] = createSignal(APP_VERSION);
  const [storage, setStorage] = createSignal<StorageStatus>({ 
    used: '-', 
    size: '-', 
    percent: '0%', 
    type: null,
    hymofs_available: false 
  });
  const [systemInfo, setSystemInfo] = createSignal<SystemInfo>({ kernel: '-', selinux: '-', mountBase: '-', activeMounts: [] });
  const [activePartitions, setActivePartitions] = createSignal<string[]>([]);
  const [conflicts, setConflicts] = createSignal<ConflictEntry[]>([]);
  const [diagnostics, setDiagnostics] = createSignal<DiagnosticIssue[]>([]);
  
  const [loadingConfig, setLoadingConfig] = createSignal(false);
  const [loadingModules, setLoadingModules] = createSignal(false);
  const [loadingLogs, setLoadingLogs] = createSignal(false);
  const [loadingStatus, setLoadingStatus] = createSignal(false);
  const [loadingConflicts, setLoadingConflicts] = createSignal(false);
  const [loadingDiagnostics, setLoadingDiagnostics] = createSignal(false);
  
  const [savingConfig, setSavingConfig] = createSignal(false);
  const [savingModules, setSavingModules] = createSignal(false);

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

  const L = createMemo(() => loadedLocale()?.default || {});

  const modeStats = createMemo((): ModeStats => {
    const stats = { auto: 0, magic: 0, hymofs: 0 };
    modules().forEach(m => {
        if (!m.is_mounted) return;
        if (m.mode === 'auto') stats.auto++;
        else if (m.mode === 'magic') stats.magic++;
        else if (m.mode === 'hymofs') stats.hymofs++;
    });
    return stats;
  });

  function showToast(text: string, type: 'info' | 'success' | 'error' = 'info') {
    const id = Date.now().toString();
    const newToast = { id, text, type, visible: true };
    setToast(newToast);
    setTimeout(() => {
      if (toast().id === id) {
        setToast(t => ({ ...t, visible: false }));
      }
    }, 3000);
  }

  function setTheme(t: 'auto' | 'light' | 'dark') {
    setThemeSignal(t);
  }

  createEffect(() => {
    const currentTheme = theme();
    const sysDark = isSystemDark();
    const currentSeed = seed();

    const isDark = currentTheme === 'auto' ? sysDark : currentTheme === 'dark';
    document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
    Monet.apply(currentSeed, isDark);
  });

  async function loadLocale(code: string) {
    const match = Object.entries(localeModules).find(([path]) => path.endsWith(`/${code}.json`));
    if (match) {
        setLoadedLocale(match[1]);
    } else {
        setLoadedLocale(localeModules['../locales/en.json']);
    }
  }

  function setLang(code: string) {
    setLangSignal(code);
    localStorage.setItem('lang', code);
    loadLocale(code);
  }

  function toggleBottomNavFix() {
    const newVal = !fixBottomNav();
    setFixBottomNavSignal(newVal);
    localStorage.setItem('hm_fix_bottom_nav', String(newVal));
    
    const dict = L();
    const msg = newVal 
        ? (dict.config?.fixBottomNavOn || 'Bottom Nav Fix Enabled') 
        : (dict.config?.fixBottomNavOff || 'Bottom Nav Fix Disabled');
    showToast(msg, 'info');
  }

  async function init() {
    const savedLang = localStorage.getItem('lang') || 'en';
    setLangSignal(savedLang);
    await loadLocale(savedLang);

    setFixBottomNavSignal(localStorage.getItem('hm_fix_bottom_nav') === 'true');

    const darkModeQuery = window.matchMedia('(prefers-color-scheme: dark)');
    setIsSystemDark(darkModeQuery.matches);
    
    darkModeQuery.addEventListener('change', (e) => {
      setIsSystemDark(e.matches);
    });

    try {
        const sysColor = await API.fetchSystemColor();
        if (sysColor) {
            setSeed(sysColor);
        }
    } catch {}
    
    await Promise.all([
      loadConfig(),
      loadStatus()
    ]);
  }

  async function loadConfig() {
    setLoadingConfig(true);
    try {
      const data = await API.loadConfig();
      setConfig(data);
    } catch (e) {
      showToast('Failed to load config', 'error');
    }
    setLoadingConfig(false);
  }

  async function saveConfig() {
    setSavingConfig(true);
    try {
      await API.saveConfig(config());
      showToast(L().common?.saved || 'Saved', 'success');
    } catch (e) {
      showToast('Failed to save config', 'error');
    }
    setSavingConfig(false);
  }

  async function resetConfig() {
    setSavingConfig(true);
    try {
      await API.resetConfig();
      await loadConfig();
      showToast(L().config?.resetSuccess || 'Config reset to defaults', 'success');
    } catch (e) {
      showToast('Failed to reset config', 'error');
    }
    setSavingConfig(false);
  }

  async function loadModules() {
    setLoadingModules(true);
    try {
      const data = await API.scanModules(config().moduledir);
      setModules(data);
    } catch (e) {
      showToast('Failed to load modules', 'error');
    }
    setLoadingModules(false);
  }

  async function saveModules() {
    setSavingModules(true);
    try {
      await API.saveModules(modules());
      showToast(L().common?.saved || 'Saved', 'success');
    } catch (e) {
      showToast('Failed to save module modes', 'error');
    }
    setSavingModules(false);
  }

  async function loadLogs(silent: boolean = false) {
    if (!silent) setLoadingLogs(true);
    try {
      const rawLogs = await API.readLogs();
      const parsed = rawLogs.split('\n').map(line => {
        const text = line.replace(/^[\d-]{10}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?\s*/, '');
        let type: LogEntry['type'] = 'info';
        if (text.includes('[E]') || text.includes('[ERROR]')) type = 'error';
        else if (text.includes('[W]') || text.includes('[WARN]')) type = 'warn';
        else if (text.includes('[D]') || text.includes('[DEBUG]')) type = 'debug';
        return { text, type };
      });
      setLogs(parsed);
    } catch (e) {
      setLogs([{ text: "Failed to load logs.", type: 'error' }]);
    }
    setLoadingLogs(false);
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
      
      setLoadingDiagnostics(true);
      const diag = await API.getDiagnostics();
      setDiagnostics(diag);
      setLoadingDiagnostics(false);

    } catch (e) {}
    setLoadingStatus(false);
  }

  async function loadConflicts() {
      setLoadingConflicts(true);
      try {
          const conf = await API.getConflicts();
          setConflicts(conf);
          if (conf.length === 0) {
              showToast(L().modules?.noConflicts || "No conflicts detected", "success");
          }
      } catch (e) {
          showToast(L().modules?.conflictError || "Failed to check conflicts", "error");
      }
      setLoadingConflicts(false);
  }

  return {
    get theme() { return theme(); },
    get isSystemDark() { return isSystemDark(); },
    get lang() { return lang(); },
    get seed() { return seed(); },
    get availableLanguages() { return availableLanguages; },
    get L() { return L(); },
    
    get toast() { return toast(); },
    get toasts() { return toast().visible ? [toast()] : []; },
    
    get fixBottomNav() { return fixBottomNav(); },
    toggleBottomNavFix,
    showToast,
    setTheme,
    setLang,
    init,
    
    get config() { return config(); },
    set config(v) { setConfig(v); },
    loadConfig,
    saveConfig,
    resetConfig,
    
    get modules() { return modules(); },
    set modules(v) { setModules(v); },
    get modeStats() { return modeStats(); },
    loadModules,
    saveModules,
    
    get logs() { return logs(); },
    loadLogs,
    
    get device() { return device(); },
    get version() { return version(); },
    get storage() { return storage(); },
    get systemInfo() { return systemInfo(); },
    get activePartitions() { return activePartitions(); },
    
    get conflicts() { return conflicts(); },
    loadConflicts,
    
    get diagnostics() { return diagnostics(); },
    
    loadStatus,
    
    get loading() {
      return {
        get config() { return loadingConfig(); },
        get modules() { return loadingModules(); },
        get logs() { return loadingLogs(); },
        get status() { return loadingStatus(); },
        get conflicts() { return loadingConflicts(); },
        get diagnostics() { return loadingDiagnostics(); }
      };
    },
    
    get saving() {
      return {
        get config() { return savingConfig(); },
        get modules() { return savingModules(); }
      };
    }
  };
};

export const store = createGlobalStore();