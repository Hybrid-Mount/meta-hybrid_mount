<script lang="ts">
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import './TopBar.css';
  import '@material/web/icon/icon.js';
  import '@material/web/iconbutton/icon-button.js';

  let showLangMenu = $state(false);
  let langButtonRef = $state<HTMLElement>();
  let menuRef = $state<HTMLDivElement>();

  function toggleTheme() {
    let nextTheme: 'light' | 'dark' | 'auto';
    let toastMsg: string;
    const common = store.L?.common;

    if (store.theme === 'auto') {
      nextTheme = 'light';
      toastMsg = common?.themeLight ?? 'Light Mode';
    } else if (store.theme === 'light') {
      nextTheme = 'dark';
      toastMsg = common?.themeDark ?? 'Dark Mode';
    } else {
      nextTheme = 'auto';
      toastMsg = common?.themeAuto ?? 'Auto Mode';
    }
    store.setTheme(nextTheme);
    store.showToast(toastMsg, 'info');
  }

  function getThemeIcon() {
    if (store.theme === 'auto') return ICONS.auto_mode;
    if (store.theme === 'light') return ICONS.light_mode;
    return ICONS.dark_mode;
  }

  function setLang(code: string) {
    store.setLang(code);
    showLangMenu = false;
  }

  function handleOutsideClick(e: MouseEvent) {
    if (showLangMenu && 
        menuRef && !menuRef.contains(e.target as Node) && 
        langButtonRef && !langButtonRef.contains(e.target as Node)) {
      showLangMenu = false;
    }
  }
</script>

<svelte:window onclick={handleOutsideClick} />

<header class="top-bar">
  <div class="top-bar-content">
    <h1 class="screen-title">{store.L?.common?.appName}</h1>
    <div class="top-actions">
      <md-icon-button 
        onclick={toggleTheme} 
        title={store.L?.common?.theme}
        role="button"
        tabindex="0"
        onkeydown={() => {}}
      >
        <md-icon>
          <svg viewBox="0 0 24 24"><path d={getThemeIcon()} /></svg>
        </md-icon>
      </md-icon-button>

      <div style="position: relative; display: inline-flex;">
        <md-icon-button 
          bind:this={langButtonRef}
          onclick={() => showLangMenu = !showLangMenu} 
          title={store.L?.common?.language}
          role="button"
          tabindex="0"
          onkeydown={() => {}}
        >
          <md-icon>
            <svg viewBox="0 0 24 24"><path d={ICONS.translate} /></svg>
          </md-icon>
        </md-icon-button>

        {#if showLangMenu}
          <div class="menu-dropdown" bind:this={menuRef}>
             {#each store.availableLanguages ?? [] as l}
              <button class="menu-item" onclick={() => setLang(l.code)}>{l.name}</button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</header>