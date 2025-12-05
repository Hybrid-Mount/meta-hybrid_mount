<script>
  import { onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { cubicOut, cubicIn } from 'svelte/easing';
  import { store } from './lib/store.svelte';
  import NavBar from './components/NavBar.svelte';
  import Toast from './components/Toast.svelte';
  
  import StatusTab from './routes/StatusTab.svelte';
  import ConfigTab from './routes/ConfigTab.svelte';
  import ModulesTab from './routes/ModulesTab.svelte';
  import LogsTab from './routes/LogsTab.svelte';
  import InfoTab from './routes/InfoTab.svelte';

  import './app.css';
  import './layout.css';

  let activeTab = $state('status');
  let transitionDirection = $state(1);
  let touchStartX = 0;
  let touchEndX = 0;
  let containerWidth = $state(0);

  const TABS = ['status', 'config', 'modules', 'logs', 'info'];

  function switchTab(id) {
    const currentIndex = TABS.indexOf(activeTab);
    const newIndex = TABS.indexOf(id);
    if (currentIndex === newIndex) return;
    transitionDirection = newIndex > currentIndex ? 1 : -1;
    activeTab = id;
  }

  function handleTouchStart(e) {
    touchStartX = e.changedTouches[0].screenX;
  }

  function handleTouchEnd(e) {
    touchEndX = e.changedTouches[0].screenX;
    
    // Use 33% of container width as threshold
    const threshold = (containerWidth * 0.33) || 80;
    const diff = touchStartX - touchEndX;
    const currentIndex = TABS.indexOf(activeTab);
    
    if (Math.abs(diff) < threshold) return;

    if (diff > 0 && currentIndex < TABS.length - 1) {
      switchTab(TABS[currentIndex + 1]);
    } else if (diff < 0 && currentIndex > 0) {
      switchTab(TABS[currentIndex - 1]);
    }
  }

  onMount(() => {
    store.init();
  });
</script>

<div class="app-root">
  <NavBar {activeTab} onTabChange={switchTab} />

  <main class="main-content" 
        bind:clientWidth={containerWidth}
        ontouchstart={handleTouchStart} 
        ontouchend={handleTouchEnd}>
    {#key activeTab}
      <div class="tab-pane" 
           in:fly={{ x: 30 * transitionDirection, duration: 250, delay: 90, easing: cubicOut }} 
           out:fly={{ x: -30 * transitionDirection, duration: 150, easing: cubicIn }}>
        
        {#if activeTab === 'status'}
          <StatusTab />
        {:else if activeTab === 'config'}
          <ConfigTab />
        {:else if activeTab === 'modules'}
          <ModulesTab />
        {:else if activeTab === 'logs'}
          <LogsTab />
        {:else if activeTab === 'info'}
          <InfoTab />
        {/if}
      </div>
    {/key}
  </main>

  <Toast />
</div>