<script lang="ts">
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import Skeleton from '../components/Skeleton.svelte';
  import BottomActions from '../components/BottomActions.svelte';
  import { API } from '../lib/api';
  import type { Module, MountMode } from '../lib/types';
  import './ModulesTab.css';

  let searchQuery = $state('');
  let filterType = $state('all');
  let expandedId = $state<string | null>(null);
  
  let initialRulesSnapshot = $state<Record<string, string>>({});

  onMount(() => {
    load();
  });

  function load() {
    store.loadModules().then(() => {
        const snapshot: Record<string, string> = {};
        store.modules.forEach(m => {
            snapshot[m.id] = JSON.stringify(m.rules);
        });
        initialRulesSnapshot = snapshot;
    });
  }

  let dirtyModules = $derived(store.modules.filter(m => {
      const initial = initialRulesSnapshot[m.id];
      if (!initial) return false;
      return JSON.stringify(m.rules) !== initial;
  }));

  let isDirty = $derived(dirtyModules.length > 0);

  async function save() {
    store.saving.modules = true;
    try {
        for (const mod of dirtyModules) {
            await API.saveModuleRules(mod.id, mod.rules);
        }
        
        await load();
        store.showToast(store.L.modules?.saveSuccess || store.L.common?.saveSuccess || "Saved successfully", 'success');
    } catch (e: any) {
        console.error(e);
        store.showToast(e.message || store.L.modules?.saveFailed || "Failed to save", 'error');
    } finally {
        store.saving.modules = false;
    }
  }

  let filteredModules = $derived(store.modules.filter(m => {
    const q = searchQuery.toLowerCase();
    const matchSearch = m.name.toLowerCase().includes(q) || m.id.toLowerCase().includes(q);
    const matchFilter = filterType === 'all' || m.mode === filterType;
    return matchSearch && matchFilter;
  }));

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }

  function handleKeydown(e: KeyboardEvent, id: string) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      toggleExpand(id);
    }
  }

  function getModeLabel(mode: string) {
      const m = store.L.modules?.modes;
      if (mode === 'magic') return m?.magic ?? 'Magic Mount';
      if (mode === 'hymofs') return m?.hymo ?? 'HymoFS';
      if (mode === 'ignore') return m?.ignore ?? 'Ignore';
      return m?.auto ?? 'OverlayFS';
  }

  function addPathRule(mod: Module) {
      if (!mod.rules.paths) mod.rules.paths = {};
      let newKey = "new/path";
      let counter = 1;
      while (newKey in mod.rules.paths) {
          newKey = `new/path${counter++}`;
      }
      mod.rules.paths[newKey] = 'magic';
      mod.rules = { ...mod.rules };
  }

  function removePathRule(mod: Module, path: string) {
      delete mod.rules.paths[path];
      mod.rules = { ...mod.rules };
  }

  function updatePathKey(mod: Module, oldPath: string, newPath: string) {
      if (oldPath === newPath) return;
      if (!newPath.trim()) return;
      
      const mode = mod.rules.paths[oldPath];
      delete mod.rules.paths[oldPath];
      mod.rules.paths[newPath] = mode;
      mod.rules = { ...mod.rules };
  }

  function updatePathMode(mod: Module, path: string, mode: MountMode) {
      mod.rules.paths[path] = mode;
      mod.rules = { ...mod.rules };
  }
</script>

<div class="md3-card desc-card">
  <p class="desc-text">
    {store.L.modules?.desc}
  </p>
</div>

<div class="search-container">
  <svg class="search-icon" viewBox="0 0 24 24"><path d={ICONS.search} /></svg>
  <input 
    type="text" 
    class="search-input" 
    placeholder={store.L.modules?.searchPlaceholder}
    bind:value={searchQuery}
  />
  <div class="filter-controls">
    <span class="filter-label-text">{store.L.modules?.filterLabel}</span>
    <select class="filter-select" bind:value={filterType}>
      <option value="all">{store.L.modules?.filterAll}</option>
      <option value="auto">{store.L.modules?.modeAuto}</option>
      <option value="magic">{store.L.modules?.modeMagic}</option>
      {#if store.storage?.hymofs_available}
          <option value="hymofs">HymoFS</option>
      {/if}
    </select>
  </div>
</div>

{#if store.loading.modules}
  <div class="rules-list">
    {#each Array(5) as _}
      <div class="rule-card">
        <div class="rule-info">
          <div class="skeleton-group">
            <Skeleton width="60%" height="20px" />
            <Skeleton width="40%" height="14px" />
          </div>
         </div>
        <Skeleton width="120px" height="40px" borderRadius="4px" />
      </div>
    {/each}
  </div>
{:else if filteredModules.length === 0}
  <div class="empty-state">
    {store.modules.length === 0 ?
      (store.L.modules?.empty ?? "No enabled modules found") : 
      "No matching modules"}
  </div>
{:else}
  <div class="rules-list">
    {#each filteredModules as mod (mod.id)}
      <div 
        class="rule-card" 
        class:expanded={expandedId === mod.id} 
        class:dirty={initialRulesSnapshot[mod.id] !== JSON.stringify(mod.rules)}
      >
        <div 
            class="rule-main"
            onclick={() => toggleExpand(mod.id)}
            onkeydown={(e) => handleKeydown(e, mod.id)}
            role="button"
            tabindex="0"
        >
          <div class="rule-info">
            <div class="info-col">
              <span class="module-name">{mod.name}</span>
              <span class="module-id">{mod.id} <span class="version-tag">{mod.version}</span></span>
            </div>
          </div>
          
          <div class="mode-badge {mod.rules.default_mode === 'magic' ? 
               'badge-magic' : mod.rules.default_mode === 'hymofs' ? 'badge-hymofs' : 'badge-auto'}"
               style:background-color={mod.rules.default_mode === 'hymofs' ?
               'var(--md-sys-color-primary-container)' : ''}
               style:color={mod.rules.default_mode === 'hymofs' ?
               'var(--md-sys-color-on-primary-container)' : ''}>
            {getModeLabel(mod.rules.default_mode)}
          </div>
        </div>
        
        {#if expandedId === mod.id}
          <div class="rule-details" transition:slide={{ duration: 200 }}>
            <p class="module-desc">{mod.description || (store.L.modules?.noDesc ?? 'No description')}</p>
            <p class="module-meta">{store.L.modules?.author ?? 'Author'}: {mod.author || (store.L.modules?.unknown ?? 'Unknown')}</p>
            
            <div class="config-section">
              
              <div class="config-row">
                <span class="config-label">{store.L.modules?.defaultMode ?? 'Default Strategy'}:</span>
                <div class="text-field compact-select">
                  <select 
                    bind:value={mod.rules.default_mode}
                    onclick={(e) => e.stopPropagation()}
                  >
                    <option value="overlay">{store.L.modules?.modes?.auto ?? 'OverlayFS (Auto)'}</option>
                    <option value="magic">{store.L.modules?.modes?.magic ?? 'Magic Mount'}</option>
                    {#if store.storage?.hymofs_available}
                      <option value="hymofs">{store.L.modules?.modes?.hymo ?? 'HymoFS'}</option>
                    {/if}
                    <option value="ignore">{store.L.modules?.modes?.ignore ?? 'Disable (Ignore)'}</option>
                  </select>
                </div>
              </div>

              <div class="paths-editor">
                 <div class="paths-header">
                     <span class="config-label">{store.L.modules?.pathRules ?? 'Path Overrides'}:</span>
                     <button class="btn-icon add-rule" onclick={() => addPathRule(mod)} title={store.L.modules?.addRule ?? 'Add Rule'}>
                         <svg viewBox="0 0 24 24" width="20" height="20"><path d={ICONS.add} fill="currentColor"/></svg>
                     </button>
                 </div>
                 
                 {#if mod.rules.paths && Object.keys(mod.rules.paths).length > 0}
                    <div class="path-list">
                        {#each Object.entries(mod.rules.paths) as [path, mode]}
                            <div class="path-row">
                                <input 
                                    type="text" 
                                    class="path-input" 
                                    value={path} 
                                    onchange={(e) => updatePathKey(mod, path, e.currentTarget.value)}
                                    placeholder={store.L.modules?.placeholder ?? "e.g. system/fonts"}
                                />
                                <select 
                                    class="path-mode-select"
                                    value={mode}
                                    onchange={(e) => updatePathMode(mod, path, e.currentTarget.value as MountMode)}
                                >
                                    <option value="overlay">{store.L.modules?.modes?.short?.auto ?? 'Overlay'}</option>
                                    <option value="magic">{store.L.modules?.modes?.short?.magic ?? 'Magic'}</option>
                                    {#if store.storage?.hymofs_available}
                                        <option value="hymofs">{store.L.modules?.modes?.short?.hymo ?? 'HymoFS'}</option>
                                    {/if}
                                    <option value="ignore">{store.L.modules?.modes?.short?.ignore ?? 'Ignore'}</option>
                                </select>
                                <button class="btn-icon delete" onclick={() => removePathRule(mod, path)} title="Remove rule">
                                    <svg viewBox="0 0 24 24" width="18" height="18"><path d={ICONS.delete} fill="currentColor"/></svg>
                                </button>
                            </div>
                        {/each}
                    </div>
                 {:else}
                    <div class="empty-paths">{store.L.modules?.noRules ?? 'No path overrides defined.'}</div>
                 {/if}
              </div>

            </div>

          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<BottomActions>
  <button class="btn-tonal" onclick={load} disabled={store.loading.modules} title={store.L.modules?.reload}>
    <svg viewBox="0 0 24 24" width="20" height="20"><path d={ICONS.refresh} fill="currentColor"/></svg>
  </button>
  <div class="spacer"></div>
  <button class="btn-filled" onclick={save} disabled={store.saving.modules || !isDirty}>
    <svg viewBox="0 0 24 24" width="18" height="18"><path d={ICONS.save} fill="currentColor"/></svg>
    {store.saving.modules ? store.L.common?.saving : store.L.modules?.save}
  </button>
</BottomActions>