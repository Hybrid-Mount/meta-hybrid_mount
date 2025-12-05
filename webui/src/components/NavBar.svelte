<script>
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import './NavBar.css';
  let { activeTab, onTabChange } = $props();
  let showLangMenu = $state(false);
  let navContainer = $state();
  let langButtonRef = $state();
  let menuRef = $state();
  let tabRefs = $state({});
  
  const TABS = [
    { id: 'status', icon: ICONS.home },
    { id: 'config', icon: ICONS.settings },
    { id: 'modules', icon: ICONS.modules },
    { id: 'logs', icon: ICONS.description },
    { id: 'info', icon: ICONS.info }
  ];
  
  $effect(() => {
    if (activeTab && tabRefs[activeTab] && navContainer) {
      const tab = tabRefs[activeTab];
      const containerWidth = navContainer.clientWidth;
      const tabLeft = tab.offsetLeft;
      const tabWidth = tab.clientWidth;
      const scrollLeft = tabLeft - (containerWidth / 2) + (tabWidth / 2);
      
      navContainer.scrollTo({
        left: scrollLeft,
        behavior: 'smooth'
      });
    }
  });

  function toggleTheme() {
    let nextTheme;
    let toastMsg;
    if (store.theme === 'auto') {
      nextTheme = 'light';
      toastMsg = store.L.common.themeLight;
    } else if (store.theme === 'light') {
      nextTheme = 'dark';
      toastMsg = store.L.common.themeDark;
    } else {
      nextTheme = 'auto';
      toastMsg = store.L.common.themeAuto;
    }

    store.setTheme(nextTheme);
    store.showToast(toastMsg, 'info');
  }

  function getThemeIcon() {
    if (store.theme === 'auto') return ICONS.auto_mode;
    if (store.theme === 'light') return ICONS.light_mode;
    return ICONS.dark_mode;
  }

  function setLang(code) {
    store.setLang(code);
    showLangMenu = false;
  }
  
  function handleOutsideClick(e) {
    if (showLangMenu && 
        menuRef && !menuRef.contains(e.target) && 
        langButtonRef && !langButtonRef.contains(e.target)) {
      showLangMenu = false;
    }
  }
</script>

<svelte:window onclick={handleOutsideClick} />

<header class="app-bar">
  <div class="app-bar-content">
    <h1 class="screen-title">{store.L.common.appName}</h1>
    <div class="top-actions">
      <button class="btn-icon" onclick={toggleTheme} title={store.L.common.theme}>
        <svg viewBox="0 0 24 24"><path d={getThemeIcon()} fill="currentColor"/></svg>
      </button>
      <button 
        class="btn-icon" 
        bind:this={langButtonRef}
        onclick={() => showLangMenu = !showLangMenu} 
        title={store.L.common.language}
      >
       <svg viewBox="0 0 24 24"><path d={ICONS.translate} fill="currentColor"/></svg>
      </button>
    </div>
  </div>
  
  {#if showLangMenu}
    <div class="menu-dropdown" bind:this={menuRef}>
      {#each store.availableLanguages as l}
        <button class="menu-item" onclick={() => setLang(l.code)}>{l.name}</button>
      {/each}
    </div>
  {/if}

  <nav class="nav-tabs" bind:this={navContainer}>
    {#each TABS as tab}
      <button 
        class="nav-tab {activeTab === tab.id ? 'active' : ''}" 
        onclick={() => onTabChange(tab.id)}
        bind:this={tabRefs[tab.id]}
      >
        <svg viewBox="0 0 24 24"><path d={tab.icon}/></svg>
        <span>{store.L.tabs[tab.id]}</span>
      </button>
    {/each}
  </nav>
</header>