<script lang="ts">
  import { onMount } from 'svelte';
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import { BUILTIN_PARTITIONS } from '../lib/constants_gen';
  import Skeleton from '../components/Skeleton.svelte';
  import BottomActions from '../components/BottomActions.svelte';
  import './StatusTab.css';
  import '@material/web/progress/linear-progress.js';
  import '@material/web/chips/chip-set.js';
  import '@material/web/chips/filter-chip.js';
  import '@material/web/iconbutton/filled-tonal-icon-button.js';
  import '@material/web/icon/icon.js';
  import { API } from '../lib/api';
  import '@material/web/dialog/dialog.js';
  import '@material/web/button/text-button.js';

  onMount(() => {
    store.loadStatus();
  });
  
  let displayPartitions = $derived([...new Set([...BUILTIN_PARTITIONS, ...(store.config?.partitions || [])])]);
  let storageLabel = $derived(store.storage?.type === 'tmpfs' ? store.systemInfo?.mountBase : store.L?.status?.storageDesc);
  let mountedCount = $derived(store.modules?.filter(m => m.is_mounted).length ?? 0);
  let showRebootConfirm = $state(false);
  
  function getDiagColor(level: string) {
      if (level === 'Critical') return 'var(--md-sys-color-error)';
      if (level === 'Warning') return 'var(--md-sys-color-tertiary)';
      return 'var(--md-sys-color-primary)';
  }

  function getStoragePercent() {
    if (!store.storage?.percent) return 0;
    return parseFloat(store.storage.percent) / 100;
  }
</script>

<md-dialog open={showRebootConfirm} onclose={() => showRebootConfirm = false}>
  <div slot="headline">{store.L?.common?.rebootTitle ?? 'Reboot System?'}</div>
  <div slot="content">
    {store.L?.common?.rebootConfirm ?? 'Are you sure you want to reboot the device?'}
  </div>
  <div slot="actions">
    <md-text-button onclick={() => showRebootConfirm = false}>
      {store.L?.common?.cancel ?? 'Cancel'}
    </md-text-button>
    <md-text-button onclick={() => { showRebootConfirm = false; API.reboot(); }}>
      {store.L?.common?.reboot ?? 'Reboot'}
    </md-text-button>
  </div>
</md-dialog>

<BottomActions>
  <div class="spacer"></div>
  <div style="display: flex; gap: 8px; align-items: center;">
    <md-filled-tonal-icon-button 
      class="reboot-btn"
      onclick={() => showRebootConfirm = true}
      title="Reboot"
      role="button"
      tabindex="0"
      onkeydown={() => {}}
    >
      <md-icon>
        <svg viewBox="0 0 24 24"><path d={ICONS.power} /></svg>
      </md-icon>
    </md-filled-tonal-icon-button>
    
    </div>
</BottomActions>
<div class="dashboard-grid">
  <div class="storage-card">
    {#if store.loading.status}
      <div class="storage-header-row">
        <div style="display: flex; flex-direction: column; gap: 8px;">
            <Skeleton width="100px" height="24px" />
            <Skeleton width="60px" height="20px" borderRadius="12px" />
        </div>
        <Skeleton width="120px" height="64px" />
      </div>
      <div class="progress-container">
        <md-linear-progress indeterminate></md-linear-progress>
      </div>
      <div class="storage-details">
        <Skeleton width="150px" height="12px" />
        <Skeleton width="80px" height="12px" />
      </div>
    {:else}
      <div class="storage-header-row">
        <div class="storage-info-col">
            <div class="storage-label-group">
                <div class="storage-icon-circle">
                  <svg viewBox="0 0 24 24"><path d={ICONS.storage} /></svg>
                </div>
                <span class="storage-title">{store.L?.status?.storageTitle ?? 'Storage'}</span>
            </div>
            {#if store.storage?.type && store.storage.type !== 'unknown'}
              <span class="storage-type-badge {store.storage.type === 'tmpfs' ? 'type-tmpfs' : 'type-ext4'}">
                {store.storage.type?.toUpperCase()}
              </span>
             {/if}
      
        </div>
        <div class="storage-value-group">
            <span class="storage-value">{store.storage?.percent ?? '0%'}</span>
            <span class="storage-unit">Used</span>
        </div>
      </div>
      
      <div class="progress-container">
        <md-linear-progress value={getStoragePercent()}></md-linear-progress>
      </div>

      <div class="storage-details">
        <span class="detail-path">{storageLabel ?? ''}</span>
        <span class="detail-nums">{store.storage?.used} / {store.storage?.size}</span>
      </div>
    {/if}
  </div>

  <div class="stats-row">
    <div class="stat-card">
      {#if store.loading.status}
        <Skeleton width="40px" height="32px" />
        <Skeleton width="60px" height="12px" style="margin-top: 8px" />
      {:else}
        <div class="stat-value">{mountedCount}</div>
        <div class="stat-label">{store.L?.status?.moduleActive ?? 'Active Modules'}</div>
      {/if}
    </div>
    <div class="stat-card">
      {#if store.loading.status}
         <Skeleton width="40px" height="32px" />
         <Skeleton width="60px" height="12px" style="margin-top: 8px" />
      {:else}
         <div class="stat-value">{store.config?.mountsource ?? '-'}</div>
         <div class="stat-label">{store.L?.config?.mountSource ?? 'Mount Source'}</div>
      {/if}
    </div>
  </div>

  <div class="mode-card">
    <div class="mode-title">{store.L?.status?.activePartitions ?? 'Partitions'}</div>
    
    {#if store.loading.status}
      <div class="partition-grid">
        {#each Array(4) as _}
          <Skeleton width="60px" height="32px" borderRadius="8px" />
        {/each}
      </div>
    {:else}
      <md-chip-set class="partition-chips">
        {#each displayPartitions as part}
          <md-filter-chip 
            label={part} 
            selected={(store.activePartitions || []).includes(part)}
            elevated
          ></md-filter-chip>
        {/each}
      </md-chip-set>
    {/if}
  </div>

  <div class="mode-card">
    <div class="mode-title">{store.L?.status?.sysInfoTitle ?? 'System Info'}</div>
    <div class="info-grid">
      <div class="info-item">
        <span class="info-label">{store.L?.status?.kernel ?? 'Kernel'}</span>
        {#if store.loading.status}
          <Skeleton width="80%" height="16px" />
        {:else}
          <span class="info-val">{store.systemInfo?.kernel || '-'}</span>
        {/if}
      </div>
      <div class="info-item">
        <span class="info-label">{store.L?.status?.selinux ?? 'SELinux'}</span>
        {#if store.loading.status}
          <Skeleton width="40%" height="16px" />
        {:else}
          <span class="info-val">{store.systemInfo?.selinux || '-'}</span>
        {/if}
      </div>
      <div class="info-item">
        <span class="info-label">HymoFS</span>
        {#if store.loading.status}
          <Skeleton width="50%" height="16px" />
        {:else}
          <span class="info-val {store.storage?.hymofs_available ? 'text-success' : 'text-disabled'}">
            {store.storage?.hymofs_available ? `Active${store.storage.hymofs_version ? ` (v${store.storage.hymofs_version})` : ''}` : 'Not Detected'}
          </span>
        {/if}
      </div>
      <div class="info-item">
        <span class="info-label">Stealth Mode</span>
        {#if store.loading.status}
          <Skeleton width="50%" height="16px" />
        {:else}
          <span class="info-val {store.systemInfo?.nukeActive ? 'text-success' : 'text-disabled'}">
            {store.systemInfo?.nukeActive ? 'Active' : 'Inactive'}
          </span>
        {/if}
      </div>
      <div class="info-item full-width">
        <span class="info-label">{store.L?.status?.mountBase ?? 'Mount Base'}</span>
        {#if store.loading.status}
          <Skeleton width="90%" height="16px" />
        {:else}
          <span class="info-val mono">{store.systemInfo?.mountBase ?? '-'}</span>
        {/if}
      </div>
    </div>
  </div>

  <div class="mode-card">
    <div class="mode-title" style="margin-bottom: 8px;">{store.L?.status?.modeStats ?? 'Mode Stats'}</div>
    {#if store.loading.status}
      <div class="skeleton-group">
        <Skeleton width="100%" height="20px" />
        <Skeleton width="100%" height="20px" />
        <Skeleton width="100%" height="20px" />
      </div>
    {:else}
      <div class="mode-row">
        <div class="mode-name">
          <div class="dot" style="background-color: var(--md-sys-color-secondary)"></div>
          {store.L?.status?.modeAuto ?? 'Auto'}
        </div>
        <span class="mode-count">{store.modeStats?.auto ?? 0}</span>
      </div>
      <div class="mode-divider"></div>
      <div class="mode-row">
        <div class="mode-name">
          <div class="dot" style="background-color: var(--md-sys-color-tertiary)"></div>
          {store.L?.status?.modeMagic ?? 'Magic'}
        </div>
        <span class="mode-count">{store.modeStats?.magic ?? 0}</span>
      </div>
      <div class="mode-divider"></div>
      <div class="mode-row">
        <div class="mode-name">
          <div class="dot" style="background-color: var(--md-sys-color-primary)"></div>
          HymoFS
        </div>
        <span class="mode-count">{store.modeStats?.hymofs || 0}</span>
      </div>
    {/if}
  </div>

  <div class="mode-card">
      <div class="mode-title">{store.L?.status?.health ?? 'System Health'}</div>
      {#if store.loading.diagnostics}
        <div class="skeleton-group">
            <Skeleton width="100%" height="20px" />
            <Skeleton width="80%" height="20px" />
        </div>
      {:else if store.diagnostics.length === 0}
        <div class="health-message healthy">{store.L?.status?.healthy ?? 'All checks passed.'}</div>
      {:else}
        <div class="diagnostic-list">
            {#each store.diagnostics as issue}
                <div class="diagnostic-item">
                    <div class="diag-level" style="color: {getDiagColor(issue.level)}">
                        {issue.level}
                    </div>
                    <div class="diag-content">
                        <div class="diag-context">{issue.context}</div>
                        <div class="diag-message">{issue.message}</div>
                    </div>
                </div>
            {/each}
        </div>
      {/if}
  </div>
</div>

<BottomActions>
  <div class="spacer"></div>
  <div style="display: flex; gap: 8px; align-items: center;">
    <md-filled-tonal-icon-button 
      class="reboot-btn"
      onclick={() => API.reboot()}
      title="Reboot"
      role="button"
      tabindex="0"
      onkeydown={() => {}}
    >
      <md-icon>
        <svg viewBox="0 0 24 24"><path d={ICONS.power} /></svg>
      </md-icon>
    </md-filled-tonal-icon-button>
    
    <md-filled-tonal-icon-button 
      onclick={() => store.loadStatus()} 
      disabled={store.loading.status}
      title={store.L?.logs?.refresh}
      role="button"
      tabindex="0"
      onkeydown={() => {}}
    >
      <md-icon>
        <svg viewBox="0 0 24 24"><path d={ICONS.refresh} /></svg>
      </md-icon>
    </md-filled-tonal-icon-button>
  </div>
</BottomActions>