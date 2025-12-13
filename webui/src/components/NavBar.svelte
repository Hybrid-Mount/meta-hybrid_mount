<script lang="ts">
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import './NavBar.css';
  import '@material/web/icon/icon.js';
  import '@material/web/ripple/ripple.js';

  interface Props {
    activeTab: string;
    onTabChange: (id: string) => void;
  }

  let { activeTab, onTabChange }: Props = $props();
  let navContainer = $state<HTMLElement>();
  let tabRefs = $state<Record<string, HTMLButtonElement>>({});

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
</script>

<nav class="bottom-nav" bind:this={navContainer} style:padding-bottom={store.fixBottomNav ? '48px' : 'max(16px, env(safe-area-inset-bottom, 0px))'}>
  {#each TABS as tab (tab.id)}
    <button 
      class="nav-tab {activeTab === tab.id ? 'active' : ''}" 
      onclick={() => onTabChange(tab.id)}
      bind:this={tabRefs[tab.id]}
      type="button"
    >
      <md-ripple></md-ripple>
      <div class="icon-container">
        <md-icon>
          <svg viewBox="0 0 24 24">
            <path d={tab.icon} style="transition: none" />
          </svg>
        </md-icon>
      </div>
      <span class="label">{store.L.tabs[tab.id as keyof typeof store.L.tabs]}</span>
    </button>
  {/each}
</nav>