<script>
  import { onMount } from 'svelte';
  import { exec } from 'kernelsu';
  import { fly } from 'svelte/transition';
  import { cubicOut, cubicIn } from 'svelte/easing';
  import locate from './locate.json';
  import './app.css';

  const DEFAULT_CONFIG = {
    moduledir: '/data/adb/modules',
    tempdir: '',
    mountsource: 'HybridMount',
    logfile: '/data/adb/meta-hybrid/daemon.log',
    verbose: false,
    partitions: []
  };

  const CONFIG_PATH = '/data/adb/meta-hybrid/config.toml';
  const MODE_CONFIG_PATH = '/data/adb/meta-hybrid/module_mode.conf';
  const IMAGE_MNT_PATH = '/data/adb/meta-hybrid/mnt';

  const icons = {
    settings: "M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58a.49.49 0 0 0 .12-.61l-1.92-3.32a.488.488 0 0 0-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54a.484.484 0 0 0-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.56-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58a.49.49 0 0 0-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.03-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z",
    modules: "M2 4v16h20V4H2zm18 14H4V6h16v12zM6 10h12v2H6v-2zm0 4h8v2H6v-2z", 
    description: "M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8v-2h8v2zm-3-5V3.5L18.5 9H13z",
    save: "M17 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V7l-4-4zm-5 16c-1.66 0-3-1.34-3-3s1.34-3 3-3 3 1.34 3 3-1.34 3-3 3zm3-10H5V5h10v4z",
    refresh: "M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z",
    translate: "M12.87 15.07l-2.54-2.51.03-.03A17.52 17.52 0 0 0 14.07 6H17V4h-7V2H8v2H1v2h11.17C11.5 7.92 10.44 9.75 9 11.35 8.07 10.32 7.3 9.19 6.69 8h-2c.73 1.63 1.73 3.17 2.98 4.56l-5.09 5.02L4 19l5-5 3.11 3.11.76-2.04zM18.5 10h-2L12 22h2l1.12-3h4.75L21 22h2l-4.5-12zm-2.62 7l1.62-4.33L19.12 17h-3.24z",
    light_mode: "M12 7c-2.76 0-5 2.24-5 5s2.24 5 5 5 5-2.24 5-5-2.24-5-5-5zM2 13h2c.55 0 1-.45 1-1s-.45-1-1-1H2c-.55 0-1 .45-1 1s.45 1 1 1zm18 0h2c.55 0 1-.45 1-1s-.45-1-1-1h-2c-.55 0-1 .45-1 1s.45 1 1 1zM11 2v2c0 .55.45 1 1 1s1-.45 1-1V2c0-.55-.45-1-1-1s-1 .45-1 1zm0 18v2c0 .55.45 1 1 1s1-.45 1-1v-2c0-.55-.45-1-1-1s-1 .45-1 1z",
    dark_mode: "M12 3c-4.97 0-9 4.03-9 9s4.03 9 9 9 9-4.03 9-9c0-.46-.04-.92-.1-1.36-.98 1.37-2.58 2.26-4.4 2.26-2.98 0-5.4-2.42-5.4-5.4 0-1.81.89-3.42 2.26-4.4-.44-.06-.9-.1-1.36-.1z"
  };

  const Monet = {
    hexToRgb: (hex) => {
      const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
      return result ? { r: parseInt(result[1], 16), g: parseInt(result[2], 16), b: parseInt(result[3], 16) } : null;
    },
    rgbToHsl: (r, g, b) => {
      r /= 255; g /= 255; b /= 255;
      const max = Math.max(r, g, b), min = Math.min(r, g, b);
      let h, s, l = (max + min) / 2;
      if (max === min) h = s = 0;
      else {
        const d = max - min;
        s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
        switch (max) {
          case r: h = (g - b) / d + (g < b ? 6 : 0); break;
          case g: h = (b - r) / d + 2; break;
          case b: h = (r - g) / d + 4; break;
        }
        h /= 6;
      }
      return { h: h * 360, s: s * 100, l: l * 100 };
    },
    hslToHex: (h, s, l) => {
      l /= 100; const a = s * Math.min(l, 1 - l) / 100;
      const f = n => {
        const k = (n + h / 30) % 12;
        const color = l - a * Math.max(Math.min(k - 3, 9 - k, 1), -1);
        return Math.round(255 * color).toString(16).padStart(2, '0');
      };
      return `#${f(0)}${f(8)}${f(4)}`;
    },
    getTone: (baseHsl, lAdjust, sAdjust = 0, hAdjust = 0) => {
      return Monet.hslToHex((baseHsl.h + hAdjust) % 360, Math.max(0, Math.min(100, baseHsl.s + sAdjust)), lAdjust);
    },
    apply: (seedHex, isDark) => {
      if (!seedHex) return;
      const rgb = Monet.hexToRgb(seedHex);
      if (!rgb) return;
      const base = Monet.rgbToHsl(rgb.r, rgb.g, rgb.b);
      
      const p = { h: base.h, s: Math.min(base.s, 90) };
      const s = { h: base.h, s: Math.min(base.s, 40) }; 
      const t = { h: (base.h + 60) % 360, s: Math.min(base.s, 50) };
      const n = { h: base.h, s: Math.min(base.s, 10) };
      const nv = { h: base.h, s: Math.min(base.s, 15) };
      const err = { h: 350, s: 80 };

      const tones = isDark ? {
        primary: Monet.getTone(p, 80), onPrimary: Monet.getTone(p, 20),
        primaryCont: Monet.getTone(p, 30), onPrimaryCont: Monet.getTone(p, 90),
        secondary: Monet.getTone(s, 80), onSecondary: Monet.getTone(s, 20),
        secondaryCont: Monet.getTone(s, 30), onSecondaryCont: Monet.getTone(s, 90),
        tertiary: Monet.getTone(t, 80), onTertiary: Monet.getTone(t, 20),
        tertiaryCont: Monet.getTone(t, 30), onTertiaryCont: Monet.getTone(t, 90),
        error: Monet.getTone(err, 80), onError: Monet.getTone(err, 20),
        errorCont: Monet.getTone(err, 30), onErrorCont: Monet.getTone(err, 90),
        bg: Monet.getTone(n, 6), onBg: Monet.getTone(n, 90),
        surf: Monet.getTone(n, 6), onSurf: Monet.getTone(n, 90),
        surfVar: Monet.getTone(nv, 30), onSurfVar: Monet.getTone(nv, 80),
        outline: Monet.getTone(nv, 60), outlineVar: Monet.getTone(nv, 30),
        surfContLow: Monet.getTone(n, 10), surfCont: Monet.getTone(n, 12),
        surfContHigh: Monet.getTone(n, 17), surfContHighest: Monet.getTone(n, 22),
      } : {
        primary: Monet.getTone(p, 40), onPrimary: Monet.getTone(p, 100),
        primaryCont: Monet.getTone(p, 90), onPrimaryCont: Monet.getTone(p, 10),
        secondary: Monet.getTone(s, 40), onSecondary: Monet.getTone(s, 100),
        secondaryCont: Monet.getTone(s, 90), onSecondaryCont: Monet.getTone(s, 10),
        tertiary: Monet.getTone(t, 40), onTertiary: Monet.getTone(t, 100),
        tertiaryCont: Monet.getTone(t, 90), onTertiaryCont: Monet.getTone(t, 10),
        error: Monet.getTone(err, 40), onError: Monet.getTone(err, 100),
        errorCont: Monet.getTone(err, 90), onErrorCont: Monet.getTone(err, 10),
        bg: Monet.getTone(n, 99), onBg: Monet.getTone(n, 10),
        surf: Monet.getTone(n, 99), onSurf: Monet.getTone(n, 10),
        surfVar: Monet.getTone(nv, 90), onSurfVar: Monet.getTone(nv, 30),
        outline: Monet.getTone(nv, 50), outlineVar: Monet.getTone(nv, 80),
        surfContLow: Monet.getTone(n, 96), surfCont: Monet.getTone(n, 94),
        surfContHigh: Monet.getTone(n, 92), surfContHighest: Monet.getTone(n, 90),
      };

      const root = document.documentElement.style;
      root.setProperty('--md-sys-color-primary', tones.primary);
      root.setProperty('--md-sys-color-on-primary', tones.onPrimary);
      root.setProperty('--md-sys-color-primary-container', tones.primaryCont);
      root.setProperty('--md-sys-color-on-primary-container', tones.onPrimaryCont);
      root.setProperty('--md-sys-color-secondary', tones.secondary);
      root.setProperty('--md-sys-color-on-secondary', tones.onSecondary);
      root.setProperty('--md-sys-color-secondary-container', tones.secondaryCont);
      root.setProperty('--md-sys-color-on-secondary-container', tones.onSecondaryCont);
      root.setProperty('--md-sys-color-tertiary', tones.tertiary);
      root.setProperty('--md-sys-color-on-tertiary', tones.onTertiary);
      root.setProperty('--md-sys-color-tertiary-container', tones.tertiaryCont);
      root.setProperty('--md-sys-color-on-tertiary-container', tones.onTertiaryCont);
      root.setProperty('--md-sys-color-error', tones.error);
      root.setProperty('--md-sys-color-on-error', tones.onError);
      root.setProperty('--md-sys-color-error-container', tones.errorCont);
      root.setProperty('--md-sys-color-on-error-container', tones.onErrorCont);
      root.setProperty('--md-sys-color-background', tones.bg);
      root.setProperty('--md-sys-color-on-background', tones.onBg);
      root.setProperty('--md-sys-color-surface', tones.surf);
      root.setProperty('--md-sys-color-on-surface', tones.onSurf);
      root.setProperty('--md-sys-color-surface-variant', tones.surfVar);
      root.setProperty('--md-sys-color-on-surface-variant', tones.onSurfVar);
      root.setProperty('--md-sys-color-outline', tones.outline);
      root.setProperty('--md-sys-color-outline-variant', tones.outlineVar);
      root.setProperty('--md-sys-color-surface-container-low', tones.surfContLow);
      root.setProperty('--md-sys-color-surface-container', tones.surfCont);
      root.setProperty('--md-sys-color-surface-container-high', tones.surfContHigh);
      root.setProperty('--md-sys-color-surface-container-highest', tones.surfContHighest);
    }
  };

  let lang = 'en';
  let theme = 'dark';
  let systemSeedColor = null;

  $: L = locate[lang] || locate['en'];

  const availableLanguages = Object.keys(locate).map(code => ({
    code,
    name: locate[code]?.lang?.display || code.toUpperCase()
  }));

  let showLangMenu = false;
  let activeTab = 'config';
  let transitionDirection = 1;
  const TABS = [
    { id: 'config', icon: icons.settings },
    { id: 'modules', icon: icons.modules },
    { id: 'logs', icon: icons.description }
  ];

  let config = { ...DEFAULT_CONFIG };
  let partitionInput = '';
  let modules = [];

  let loading = { config: false, modules: false, logs: false };
  let saving = { config: false, modules: false };
  let messages = { text: '', type: 'info', visible: false };
  let logLines = [];

  let touchStartX = 0;
  let touchEndX = 0;

  function showMessage(msg, type='info') {
    messages = { text: msg, type, visible: true };
    setTimeout(() => { messages.visible = false; }, 3000);
  }

  function parseKvConfig(text) {
    try {
      const result = { ...DEFAULT_CONFIG };
      text.split('\n').forEach(line => {
        line = line.trim();
        if (!line || line.startsWith('#')) return;
        
        const eq = line.indexOf('=');
        if (eq < 0) return;
        
        let key = line.slice(0, eq).trim();
        let val = line.slice(eq + 1).trim();
        
        if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
          val = val.slice(1, -1);
        }
        
        if (key === 'moduledir') result.moduledir = val;
        else if (key === 'tempdir') result.tempdir = val;
        else if (key === 'mountsource') result.mountsource = val;
        else if (key === 'verbose') result.verbose = (val === 'true');
        else if (key === 'partitions') result.partitions = val.split(',').map(s => s.trim()).filter(Boolean);
      });
      return result;
    } catch (e) { return null; }
  }

  function serializeKvConfig(cfg) {
    const q = s => `"${s}"`;
    const lines = ['# Hybrid Mount Config', ''];
    lines.push(`moduledir = ${q(cfg.moduledir)}`);
    if (cfg.tempdir) lines.push(`tempdir = ${q(cfg.tempdir)}`);
    lines.push(`mountsource = ${q(cfg.mountsource)}`);
    lines.push(`verbose = ${cfg.verbose}`); 
    if (cfg.partitions.length) lines.push(`partitions = ${q(cfg.partitions.join(','))}`);
    return lines.join('\n');
  }

  async function loadConfig() {
    loading.config = true;
    try {
      const { errno, stdout } = await exec(`[ -f "${CONFIG_PATH}" ] && cat "${CONFIG_PATH}" || echo ""`);
      config = (errno === 0 && stdout.trim()) ? (parseKvConfig(stdout) || DEFAULT_CONFIG) : DEFAULT_CONFIG;
      partitionInput = config.partitions.join(', ');
      showMessage(L.config.loadSuccess);
    } catch (e) { showMessage(L.config.loadError, 'error'); }
    loading.config = false;
  }

  async function saveConfig() {
    saving.config = true;
    try {
      config.partitions = partitionInput.split(',').map(s => s.trim()).filter(Boolean);
      const data = serializeKvConfig(config).replace(/'/g, "'\\''");
      const cmd = `mkdir -p "$(dirname "${CONFIG_PATH}")" && printf '%s\n' '${data}' > "${CONFIG_PATH}"`;
      const { errno } = await exec(cmd);
      if (errno === 0) showMessage(L.config.saveSuccess);
      else showMessage(L.config.saveFailed, 'error');
    } catch (e) { showMessage(L.config.saveFailed, 'error');
    }
    saving.config = false;
  }

  async function loadModules() {
    loading.modules = true;
    modules = [];
    try {
      const { stdout: modeOut } = await exec(`[ -f "${MODE_CONFIG_PATH}" ] && cat "${MODE_CONFIG_PATH}" || echo ""`);
      const modeMap = new Map();
      modeOut.split('\n').forEach(l => {
        const [id, m] = l.split('=').map(s => s.trim());
        if (id) modeMap.set(id, m);
      });
      const dir = config.moduledir || DEFAULT_CONFIG.moduledir;
      const imgDir = IMAGE_MNT_PATH;
      const cmd = `
        cd "${dir}" && for d in *;
        do
          if [ -d "$d" ] && [ ! -f "$d/disable" ] && [ ! -f "$d/skip_mount" ] && [ ! -f "$d/remove" ];
          then
             HAS_CONTENT=false
             if [ -d "$d/system" ] || [ -d "$d/vendor" ] || [ -d "$d/product" ] || [ -d "$d/system_ext" ] || [ -d "$d/odm" ] || [ -d "$d/oem" ]; then
               HAS_CONTENT=true
             fi
             if [ "$HAS_CONTENT" = "false" ];
             then
                if [ -d "${imgDir}/$d/system" ] || [ -d "${imgDir}/$d/vendor" ] || [ -d "${imgDir}/$d/product" ] || [ -d "${imgDir}/$d/system_ext" ] || [ -d "${imgDir}/$d/odm" ] || [ -d "${imgDir}/$d/oem" ]; then
                  HAS_CONTENT=true
                fi
             fi
             if [ "$HAS_CONTENT" = "true" ];
             then echo "$d"; fi
          fi
        done
      `;
      const { errno, stdout } = await exec(cmd);
      if (errno === 0) {
        modules = stdout.split('\n')
          .map(s => s.trim())
          .filter(s => s && !['meta-hybrid', 'meta-overlayfs', 'magic_mount'].includes(s))
          .map(id => ({ id, mode: modeMap.get(id) || 'auto' }));
      } else {
        showMessage(L.modules.scanError, 'error');
      }
    } catch (e) { showMessage(L.modules.scanError, 'error'); }
    loading.modules = false;
  }

  async function saveModules() {
    saving.modules = true;
    try {
      let content = "# Module Modes\n";
      modules.forEach(m => { if (m.mode !== 'auto') content += `${m.id}=${m.mode}\n`; });
      const data = content.replace(/'/g, "'\\''");
      const { errno } = await exec(`mkdir -p "$(dirname "${MODE_CONFIG_PATH}")" && printf '%s\n' '${data}' > "${MODE_CONFIG_PATH}"`);
      if (errno === 0) showMessage(L.modules.saveSuccess);
      else showMessage(L.modules.saveFailed, 'error');
    } catch (e) { showMessage(L.modules.saveFailed, 'error');
    }
    saving.modules = false;
  }

  function parseLogs(raw) {
    if (!raw || raw.includes('No such file')) return [];
    return raw.split('\n').map(line => {
      let text = line;
      let type = 'debug';

      if (text.includes('[ERROR]')) type = 'error';
      else if (text.includes('[WARN]')) type = 'warn';
      else if (text.includes('[INFO]')) type = 'info';

      return { text, type };
    });
  }

  async function loadLog() {
    loading.logs = true;
    logLines = [];
    try {
      const f = config.logfile || DEFAULT_CONFIG.logfile;
      const { errno, stdout, stderr } = await exec(`[ -f "${f}" ] && cat "${f}" || echo ""`);
      if (errno === 0 && stdout) {
        logLines = parseLogs(stdout);
      } else {
        logLines = [{ text: stdout || stderr || L.logs.empty, type: 'debug' }];
      }
    } catch (e) { 
      showMessage(L.logs.readException, 'error');
    }
    loading.logs = false;
  }

  async function fetchSystemColor() {
    try {
      const { stdout } = await exec('settings get secure theme_customization_overlay_packages');
      if (!stdout) return;
      const match = /["']?android\.theme\.customization\.system_palette["']?\s*:\s*["']?#?([0-9a-fA-F]{6,8})["']?/i.exec(stdout) || 
                    /["']?source_color["']?\s*:\s*["']?#?([0-9a-fA-F]{6,8})["']?/i.exec(stdout);
      if (match && match[1]) {
        let hex = match[1];
        if (hex.length === 8) hex = hex.substring(2);
        systemSeedColor = '#' + hex;
        Monet.apply(systemSeedColor, theme === 'dark');
      }
    } catch (e) {}
  }

  function handleTouchStart(e) {
    touchStartX = e.changedTouches[0].screenX;
  }

  function handleTouchEnd(e) {
    touchEndX = e.changedTouches[0].screenX;
    handleSwipe();
  }

  function switchTab(id) {
    const currentIndex = TABS.findIndex(t => t.id === activeTab);
    const newIndex = TABS.findIndex(t => t.id === id);
    if (currentIndex === newIndex) return;
    transitionDirection = newIndex > currentIndex ? 1 : -1;
    activeTab = id;
  }

  function handleSwipe() {
    const threshold = 50;
    const diff = touchStartX - touchEndX;
    const currentIndex = TABS.findIndex(t => t.id === activeTab);
    
    if (Math.abs(diff) < threshold) return;

    if (diff > 0) {
      if (currentIndex < TABS.length - 1) {
        transitionDirection = 1;
        activeTab = TABS[currentIndex + 1].id;
      }
    } else {
      if (currentIndex > 0) {
        transitionDirection = -1;
        activeTab = TABS[currentIndex - 1].id;
      }
    }
  }

  onMount(() => {
    const savedLang = localStorage.getItem('mm-lang');
    if (savedLang && locate[savedLang]) lang = savedLang;
    
    const savedTheme = localStorage.getItem('mm-theme');
    setTheme(savedTheme || (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'));
    
    loadConfig();
    fetchSystemColor();
  });

  function setTheme(t) {
    theme = t;
    document.documentElement.setAttribute('data-theme', t);
    localStorage.setItem('mm-theme', t);
    if (systemSeedColor) Monet.apply(systemSeedColor, t === 'dark');
  }

  function toggleTheme() { setTheme(theme === 'light' ? 'dark' : 'light'); }
  
  $: if (activeTab === 'modules') loadModules();
  $: if (activeTab === 'logs') loadLog();
</script>

<div class="app-root">
  <header class="app-bar">
    <div class="app-bar-content">
      <h1 class="screen-title">{L.common.appName}</h1>
      <div class="top-actions">
        <button class="btn-icon" on:click={toggleTheme} title={L.common.theme}>
          <svg viewBox="0 0 24 24"><path d={theme === 'light' ? icons.dark_mode : icons.light_mode} fill="currentColor"/></svg>
        </button>
        <button class="btn-icon" on:click={() => showLangMenu = !showLangMenu} title={L.common.language}>
          <svg viewBox="0 0 24 24"><path d={icons.translate} fill="currentColor"/></svg>
        </button>
      </div>
    </div>
    
    {#if showLangMenu}
      <div class="menu-dropdown">
        {#each availableLanguages as l}
          <button class="menu-item" on:click={() => { lang = l.code; showLangMenu = false; localStorage.setItem('mm-lang', l.code); }}>{l.name}</button>
        {/each}
      </div>
    {/if}

    <nav class="nav-tabs">
      {#each TABS as tab}
        <button class="nav-tab {activeTab === tab.id ? 'active' : ''}" on:click={() => switchTab(tab.id)}>
          <svg viewBox="0 0 24 24"><path d={tab.icon}/></svg>
          {L.tabs[tab.id]}
        </button>
      {/each}
    </nav>
  </header>

  <main class="main-content" on:touchstart={handleTouchStart} on:touchend={handleTouchEnd}>
    {#key activeTab}
      <div class="tab-pane" 
           in:fly={{ x: 50 * transitionDirection, duration: 300, easing: cubicOut }} 
           out:fly={{ x: -50 * transitionDirection, duration: 300, easing: cubicIn }}>
        
        {#if activeTab === 'config'}
          <div class="md3-card">
            <div class="switch-row">
              <span>{L.config.verboseLabel}</span>
              <label class="md3-switch">
                <input type="checkbox" bind:checked={config.verbose}>
                <span class="track"><span class="thumb"></span></span>
              </label>
            </div>
          </div>

          <div class="md3-card">
            <div class="text-field">
              <input type="text" id="c-moduledir" bind:value={config.moduledir} placeholder={DEFAULT_CONFIG.moduledir} />
              <label for="c-moduledir">{L.config.moduleDir}</label>
            </div>
            <div class="text-field">
              <input type="text" id="c-tempdir" bind:value={config.tempdir} placeholder={L.config.autoPlaceholder} />
              <label for="c-tempdir">{L.config.tempDir}</label>
            </div>
            <div class="text-field">
              <input type="text" id="c-mountsource" bind:value={config.mountsource} placeholder={DEFAULT_CONFIG.mountsource} />
              <label for="c-mountsource">{L.config.mountSource}</label>
            </div>
            <div class="text-field">
              <input type="text" id="c-partitions" bind:value={partitionInput} placeholder="mi_ext, my_stock" />
              <label for="c-partitions">{L.config.partitions}</label>
            </div>
          </div>
        
        {:else if activeTab === 'modules'}
          <div class="md3-card" style="padding: 16px;">
            <p style="margin: 0; font-size: 14px; color: var(--md-sys-color-on-surface-variant); line-height: 1.5;">
              {L.modules.desc}
            </p>
          </div>

          {#if modules.length === 0}
            <div style="text-align:center; padding: 40px; opacity: 0.6">
              {loading.modules ? L.modules.scanning : L.modules.empty}
            </div>
          {:else}
            <div class="rules-list">
              {#each modules as mod (mod.id)}
                <div class="rule-card">
                  <div class="rule-info">
                    <div class="module-icon">{mod.id.slice(0,2)}</div>
                    <div class="module-name">{mod.id}</div>
                  </div>
                  <div class="text-field" style="margin-bottom:0; width: 140px; flex-shrink: 0;">
                    <select bind:value={mod.mode}>
                      <option value="auto">{L.modules.modeAuto}</option>
                      <option value="magic">{L.modules.modeMagic}</option>
                    </select>
                  </div>
                </div>
              {/each}
            </div>
          {/if}

        {:else if activeTab === 'logs'}
          <div class="log-container">
            {#if loading.logs}
              <div style="padding: 20px; text-align: center;">{L.logs.loading}</div>
            {:else if logLines.length === 0}
              <div style="padding: 20px; text-align: center;">{L.logs.empty}</div>
            {:else}
              {#each logLines as line}
                <span class="log-entry">
                  <span class="log-{line.type}">{line.text}</span>
                </span>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/key}
  </main>

  {#if activeTab === 'config'}
    <div class="bottom-actions" in:fly={{ y: 20, duration: 300 }}>
      <button class="btn-tonal" on:click={loadConfig} disabled={loading.config}>{L.config.reload}</button>
      <button class="btn-filled" on:click={saveConfig} disabled={saving.config}>
        <svg viewBox="0 0 24 24" width="18" height="18"><path d={icons.save} fill="currentColor"/></svg>
        {saving.config ? L.common.saving : L.config.save}
      </button>
    </div>
  {:else if activeTab === 'modules'}
    <div class="bottom-actions" in:fly={{ y: 20, duration: 300 }}>
      <button class="btn-tonal" on:click={loadModules} disabled={loading.modules} title={L.modules.reload}>
        <svg viewBox="0 0 24 24" width="20" height="20"><path d={icons.refresh} fill="currentColor"/></svg>
      </button>
      <button class="btn-filled" on:click={saveModules} disabled={saving.modules}>
        <svg viewBox="0 0 24 24" width="18" height="18"><path d={icons.save} fill="currentColor"/></svg>
        {saving.modules ? L.common.saving : L.modules.save}
      </button>
    </div>
  {:else if activeTab === 'logs'}
    <div class="bottom-actions" in:fly={{ y: 20, duration: 300 }}>
      <button class="btn-filled" on:click={loadLog} disabled={loading.logs}>
        <svg viewBox="0 0 24 24" width="18" height="18"><path d={icons.refresh} fill="currentColor"/></svg>
        {L.logs.refresh}
      </button>
    </div>
  {/if}

  {#if messages.visible}
    <div class="msg-toast">{messages.text}</div>
  {/if}
</div>