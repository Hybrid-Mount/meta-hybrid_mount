<script>
  import { onMount } from 'svelte';
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import './StatusTab.css';

  onMount(() => {
    store.loadStatus();
  });
</script>

<div class="dashboard-grid">
  <div class="storage-card">
    <div class="storage-header">
      <div style="display:flex; align-items:center; gap:8px;">
        <span class="storage-title">{store.L.status.storageTitle}</span>
        
        {#if store.storage.type && store.storage.type !== 'unknown'}
          <span class="storage-type-badge {store.storage.type === 'tmpfs' ? 'type-tmpfs' : 'type-ext4'}">
            {store.storage.type.toUpperCase()}
          </span>
        {/if}
      </div>
      
      <div class="storage-value">
        {store.storage.percent}
      </div>
    </div>
    
    <div class="progress-track">
      <div class="progress-fill" style="width: {store.storage.percent}"></div>
    </div>

    <div class="storage-details">
      <span>{store.L.status.storageDesc}</span>
      <span>{store.storage.used} / {store.storage.size}</span>
    </div>
  </div>

  <div class="stats-row">
    <div class="stat-card">
      <div class="stat-value">{store.modules.length}</div>
      <div class="stat-label">{store.L.status.moduleActive}</div>
    </div>
    <div class="stat-card">
      <div class="stat-value">{store.config.mountsource}</div>
      <div class="stat-label">{store.L.config.mountSource}</div>
    </div>
  </div>

  <div class="mode-card">
    <div class="storage-title" style="margin-bottom: 8px;">{store.L.status.modeStats}</div>
    
    <div class="mode-row">
      <div class="mode-name">
        <div class="dot" style="background-color: var(--md-sys-color-primary)"></div>
        {store.L.status.modeAuto}
      </div>
      <span class="mode-count">{store.modeStats.auto}</span>
    </div>

    <div style="height: 1px; background-color: var(--md-sys-color-outline-variant); opacity: 0.5;"></div>

    <div class="mode-row">
      <div class="mode-name">
        <div class="dot" style="background-color: var(--md-sys-color-tertiary)"></div>
        {store.L.status.modeMagic}
      </div>
      <span class="mode-count">{store.modeStats.magic}</span>
    </div>
  </div>
</div>

<div class="bottom-actions">
  <div style="flex:1"></div>
  <button 
    class="btn-tonal" 
    onclick={() => store.loadStatus()} 
    disabled={store.loading.status}
    title={store.L.logs.refresh}
  >
    <svg viewBox="0 0 24 24" width="20" height="20"><path d={ICONS.refresh} fill="currentColor"/></svg>
  </button>
</div>