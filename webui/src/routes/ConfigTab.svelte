<script>
  import { store } from '../lib/store.svelte';
  import { ICONS, DEFAULT_CONFIG } from '../lib/constants';
  import './ConfigTab.css';

  let partitionInput = $state('');
  let initialConfigStr = $state('');

  // Validation Helpers
  const isValidPath = (p) => !p || (p.startsWith('/') && p.length > 1);
  
  let invalidModuleDir = $derived(!isValidPath(store.config.moduledir));
  let invalidTempDir = $derived(store.config.tempdir && !isValidPath(store.config.tempdir));

  // Dirty state checking
  let isDirty = $derived.by(() => {
    if (!initialConfigStr) return false;
    const currentPartitions = partitionInput.split(',').map(s => s.trim()).filter(Boolean);
    const currentConfig = { ...store.config, partitions: currentPartitions };
    return JSON.stringify(currentConfig) !== initialConfigStr;
  });

  // Init snapshot on load
  $effect(() => {
    if (!store.loading.config && store.config) {
      if (!initialConfigStr || initialConfigStr === JSON.stringify(DEFAULT_CONFIG)) {
        partitionInput = store.config.partitions.join(', ');
        initialConfigStr = JSON.stringify(store.config);
      }
    }
  });

  function save() {
    if (invalidModuleDir || invalidTempDir) {
      store.showToast(store.L.config.invalidPath, "error");
      return;
    }
    store.config.partitions = partitionInput.split(',').map(s => s.trim()).filter(Boolean);
    store.saveConfig().then(() => {
        initialConfigStr = JSON.stringify(store.config);
    });
  }
  
  function reload() {
    store.loadConfig().then(() => {
        partitionInput = store.config.partitions.join(', ');
        initialConfigStr = JSON.stringify(store.config);
    });
  }

  function resetTempDir() {
    store.config.tempdir = "";
  }
</script>

<div class="md3-card">
  <div class="switch-row">
    <span>{store.L.config.verboseLabel}</span>
    <label class="md3-switch">
      <input type="checkbox" bind:checked={store.config.verbose}>
      <span class="track"><span class="thumb"></span></span>
    </label>
  </div>
  
  <div class="switch-row">
    <span>{store.L.config.umountLabel}</span>
    <label class="md3-switch">
      <input type="checkbox" bind:checked={store.config.umount}>
      <span class="track"><span class="thumb"></span></span>
    </label>
  </div>
</div>

<div class="md3-card">
  <div class="text-field" class:error={invalidModuleDir}>
    <input type="text" id="c-moduledir" bind:value={store.config.moduledir} placeholder={DEFAULT_CONFIG.moduledir} />
    <label for="c-moduledir">{store.L.config.moduleDir}</label>
  </div>
  
  <div class="text-field" class:error={invalidTempDir} style="display:flex; align-items:center;">
    <input type="text" id="c-tempdir" bind:value={store.config.tempdir} placeholder={store.L.config.autoPlaceholder} />
    <label for="c-tempdir">{store.L.config.tempDir}</label>
    
    {#if store.config.tempdir}
      <button class="icon-reset" onclick={resetTempDir} title={store.L.config.reset}>
        ✕
      </button>
    {/if}
  </div>
  
  <div class="text-field">
    <input type="text" id="c-mountsource" bind:value={store.config.mountsource} placeholder={DEFAULT_CONFIG.mountsource} />
    <label for="c-mountsource">{store.L.config.mountSource}</label>
  </div>
  
  <div class="text-field">
    <input type="text" id="c-partitions" bind:value={partitionInput} placeholder="mi_ext, my_stock" />
    <label for="c-partitions">{store.L.config.partitions}</label>
  </div>
</div>

<div class="bottom-actions">
  <button 
    class="btn-tonal" 
    onclick={reload}
    disabled={store.loading.config}
    title={store.L.config.reload}
  >
    <svg viewBox="0 0 24 24" width="20" height="20"><path d={ICONS.refresh} fill="currentColor"/></svg>
  </button>
  <button class="btn-filled" onclick={save} disabled={store.saving.config || !isDirty}>
    <svg viewBox="0 0 24 24" width="18" height="18"><path d={ICONS.save} fill="currentColor"/></svg>
    {store.saving.config ? store.L.common.saving : store.L.config.save}
  </button>
</div>