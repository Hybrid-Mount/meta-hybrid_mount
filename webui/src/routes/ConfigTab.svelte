<script lang="ts">
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import ChipInput from '../components/ChipInput.svelte';
  import BottomActions from '../components/BottomActions.svelte';
  import { slide } from 'svelte/transition';
  import './ConfigTab.css';
  import '@material/web/textfield/outlined-text-field.js';
  import '@material/web/button/filled-button.js';
  import '@material/web/iconbutton/filled-tonal-icon-button.js';
  import '@material/web/iconbutton/icon-button.js';
  import '@material/web/icon/icon.js';
  import '@material/web/ripple/ripple.js';

  let initialConfigStr = $state('');
  const isValidPath = (p: string) => !p || (p.startsWith('/') && p.length > 1);
  let invalidModuleDir = $derived(!isValidPath(store.config.moduledir));
  let invalidTempDir = $derived(store.config.tempdir && !isValidPath(store.config.tempdir));
  let isDirty = $derived.by(() => {
    if (!initialConfigStr) return false;
    return JSON.stringify(store.config) !== initialConfigStr;
  });

  $effect(() => {
    if (!store.loading.config && store.config) {
      if (!initialConfigStr || initialConfigStr === JSON.stringify(store.config)) {
        initialConfigStr = JSON.stringify(store.config);
      }
    }
  });

  $effect(() => {
    if (store.systemInfo?.zygisksuEnforce && store.systemInfo.zygisksuEnforce !== '0' && !store.config.allow_umount_coexistence) {
        if (!store.config.disable_umount) {
            store.config.disable_umount = true;
        }
    }
  });

  function save() {
    if (invalidModuleDir || invalidTempDir) {
      store.showToast(store.L.config.invalidPath, "error");
      return;
    }
    store.saveConfig().then(() => {
        initialConfigStr = JSON.stringify(store.config);
    });
  }

  function reload() {
    store.loadConfig().then(() => {
        initialConfigStr = JSON.stringify(store.config);
    });
  }

  function resetTempDir() {
    store.config.tempdir = "";
  }

  function toggle(key: keyof typeof store.config) {
    if (key === 'disable_umount') {
       if (store.systemInfo?.zygisksuEnforce && store.systemInfo.zygisksuEnforce !== '0' && !store.config.allow_umount_coexistence) {
          store.showToast(store.L.config?.coexistenceRequired || "Coexistence required", "error");
          return;
       }
    }
    if (typeof store.config[key] === 'boolean') {
      (store.config as any)[key] = !store.config[key];
    }
  }

  function handleInput(e: Event, key: keyof typeof store.config) {
    const target = e.target as HTMLInputElement;
    (store.config as any)[key] = target.value;
  }
</script>

<div class="config-container">
  <section class="config-group">
    <div class="config-card storage-card-fixed">
      <div class="card-header">
        <div class="card-icon">
          <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.storage} /></svg></md-icon>
        </div>
        <div class="card-text">
          <span class="card-title">{store.L.status?.storageTitle ?? 'Storage'}</span>
          <span class="card-desc">Configure module path</span>
        </div>
      </div>
      
      <div class="input-stack">
        <md-outlined-text-field 
          label={store.L.config.moduleDir} 
          value={store.config.moduledir}
          oninput={(e) => handleInput(e, 'moduledir')}
          error={invalidModuleDir}
          class="full-width-field"
        >
          <md-icon slot="leading-icon"><svg viewBox="0 0 24 24"><path d={ICONS.modules} /></svg></md-icon>
        </md-outlined-text-field>
      </div>
    </div>
  </section>

  <section class="config-group">
    <div class="config-card storage-card-fixed">
      <div class="card-header">
        <div class="card-icon">
          <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.timer} /></svg></md-icon>
        </div>
        <div class="card-text">
          <span class="card-title">{store.L.config.tempDir}</span>
          <span class="card-desc">Configure temporary path</span>
        </div>
      </div>

      <div class="input-stack">
        <md-outlined-text-field 
          label={store.L.config.tempDir} 
          value={store.config.tempdir ?? ''}
          oninput={(e) => handleInput(e, 'tempdir')}
          placeholder={store.L.config.autoPlaceholder}
          error={invalidTempDir}
          class="full-width-field"
        >
          <md-icon slot="leading-icon"><svg viewBox="0 0 24 24"><path d={ICONS.timer} /></svg></md-icon>
          {#if store.config.tempdir}
            <md-icon-button 
                slot="trailing-icon" 
                onclick={resetTempDir}
                role="button"
                tabindex="0"
                onkeydown={() => {}}
            >
              <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.close} /></svg></md-icon>
            </md-icon-button>
          {/if}
        </md-outlined-text-field>
      </div>
    </div>
  </section>

  <section class="config-group">
    <div class="config-card">
      <div class="card-header">
        <div class="card-icon">
          <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.storage} /></svg></md-icon>
        </div>
        <div class="card-text">
          <span class="card-title">{store.L.config.partitions}</span>
          <span class="card-desc">Add partitions to mount</span>
        </div>
      </div>
      <div class="p-input">
        <ChipInput bind:values={store.config.partitions} placeholder="e.g. product, system_ext..." />
      </div>
    </div>
  </section>

  <section class="config-group">
    <div class="options-grid">
      <div class="option-tile static-input">
        <div class="tile-top">
          <div class="tile-icon neutral">
            <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.ksu} /></svg></md-icon>
          </div>
        </div>
        <div class="tile-bottom">
          <span class="tile-label">{store.L.config.mountSource}</span>
          <input class="tile-input-overlay" type="text" bind:value={store.config.mountsource} />
        </div>
      </div>

      <button 
        class="option-tile clickable secondary" 
        class:active={store.config.force_ext4} 
        onclick={() => toggle('force_ext4')}
      >
        <md-ripple></md-ripple>
        <div class="tile-top">
           <div class="tile-icon">
            <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.save} /></svg></md-icon>
          </div>
        </div>
        <div class="tile-bottom">
          <span class="tile-label">{store.L.config.forceExt4}</span>
        </div>
      </button>

      <button 
        class="option-tile clickable error" 
        class:active={store.config.enable_nuke} 
        onclick={() => toggle('enable_nuke')}
      >
        <md-ripple></md-ripple>
        <div class="tile-top">
          <div class="tile-icon">
            <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.cat_paw} /></svg></md-icon>
          </div>
        </div>
        <div class="tile-bottom">
          <span class="tile-label">{store.L.config.enableNuke}</span>
        </div>
      </button>

      <button 
        class="option-tile clickable tertiary" 
        class:active={store.config.disable_umount} 
        onclick={() => toggle('disable_umount')}
      >
        <md-ripple></md-ripple>
        <div class="tile-top">
          <div class="tile-icon">
             <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.anchor} /></svg></md-icon>
          </div>
        </div>
        <div class="tile-bottom">
          <span class="tile-label">{store.L.config.disableUmount}</span>
        </div>
      </button>

      {#if store.systemInfo?.zygisksuEnforce && store.systemInfo.zygisksuEnforce !== '0'}
        <button 
          class="option-tile clickable error" 
          class:active={store.config.allow_umount_coexistence} 
          onclick={() => toggle('allow_umount_coexistence')}
          transition:slide
        >
          <md-ripple></md-ripple>
          <div class="tile-top">
            <div class="tile-icon">
              <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.shield} /></svg></md-icon>
            </div>
          </div>
          <div class="tile-bottom">
              <span class="tile-label">{store.L.config?.allowUmountCoexistence || 'Allow Coexistence'}</span>
          </div>
        </button>
      {/if}

      <button 
        class="option-tile clickable primary" 
        class:active={store.config.verbose} 
        onclick={() => toggle('verbose')}
      >
        <md-ripple></md-ripple>
        <div class="tile-top">
          <div class="tile-icon">
             <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.description} /></svg></md-icon>
          </div>
        </div>
        <div class="tile-bottom">
          <span class="tile-label">{store.L.config.verboseLabel}</span>
        </div>
      </button>

      {#if store.config.verbose}
        <button 
          class="option-tile clickable secondary" 
          class:active={store.config.dry_run} 
          onclick={() => toggle('dry_run')}
          transition:slide
        >
          <md-ripple></md-ripple>
          <div class="tile-top">
            <div class="tile-icon">
              <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.ghost} /></svg></md-icon>
             </div>
          </div>
          <div class="tile-bottom">
            <span class="tile-label">{store.L.config.dryRun}</span>
          </div>
        </button>
      {/if}
    </div>
  </section>

  <section class="config-group">
    <div class="webui-label">
        {store.L.config?.webui || 'WebUI'}
    </div>
    <div class="options-grid">
      <button 
        class="option-tile clickable secondary" 
        class:active={store.fixBottomNav} 
        onclick={store.toggleBottomNavFix}
      >
        <md-ripple></md-ripple>
        <div class="tile-top">
          <div class="tile-icon">
            <md-icon><svg viewBox="0 0 24 24"><path d="M21 5v14H3V5h18zm0-2H3c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM8 17h5v-6H8v6zm0-8h5V7H8v2zM6 17h2V7H6v10zm12-6h-2v6h2v-6zm0-4h-2v2h2V7z" /></svg></md-icon>
          </div>
        </div>
        <div class="tile-bottom">
          <span class="tile-label">{store.L.config?.fixBottomNav || 'Fix Bottom Nav'}</span>
        </div>
      </button>
    </div>
  </section>
</div>

<BottomActions>
  <md-filled-tonal-icon-button 
    onclick={reload}
    disabled={store.loading.config}
    title={store.L.config.reload}
    role="button"
    tabindex="0"
    onkeydown={() => {}}
  >
    <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.refresh} /></svg></md-icon>
  </md-filled-tonal-icon-button>

  <div class="spacer"></div>

  <md-filled-button 
    onclick={save} 
    disabled={store.saving.config || !isDirty}
    role="button"
    tabindex="0"
    onkeydown={() => {}}
  >
    <md-icon slot="icon"><svg viewBox="0 0 24 24"><path d={ICONS.save} /></svg></md-icon>
    {store.saving.config ? store.L.common.saving : store.L.config.save}
  </md-filled-button>
</BottomActions>