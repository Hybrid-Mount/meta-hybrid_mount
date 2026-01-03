/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createEffect, createMemo, Show, For } from 'solid-js';
import { store } from '../lib/store';
import { ICONS } from '../lib/constants';
import { API } from '../lib/api';
import ChipInput from '../components/ChipInput';
import BottomActions from '../components/BottomActions';
import './ConfigTab.css';
import '@material/web/textfield/outlined-text-field.js';
import '@material/web/button/filled-button.js';
import '@material/web/iconbutton/filled-tonal-icon-button.js';
import '@material/web/iconbutton/icon-button.js';
import '@material/web/icon/icon.js';
import '@material/web/ripple/ripple.js';
import '@material/web/dialog/dialog.js';
import '@material/web/button/text-button.js';
import '@material/web/switch/switch.js';
import type { OverlayMode, AppConfig } from '../lib/types';

export default function ConfigTab() {
  const [initialConfigStr, setInitialConfigStr] = createSignal('');
  const [showResetConfirm, setShowResetConfirm] = createSignal(false);

  const isValidPath = (p: string) => !p || (p.startsWith('/') && p.length > 1);
  const invalidModuleDir = createMemo(() => !isValidPath(store.config.moduledir));

  const isDirty = createMemo(() => {
    if (!initialConfigStr()) return false;
    return JSON.stringify(store.config) !== initialConfigStr();
  });

  createEffect(() => {
    if (!store.loading.config && store.config) {
      if (!initialConfigStr() || initialConfigStr() === JSON.stringify(store.config)) {
        setInitialConfigStr(JSON.stringify(store.config));
      }
    }
  });

  createEffect(() => {
    if (store.systemInfo?.zygisksuEnforce && store.systemInfo.zygisksuEnforce !== '0' && !store.config.allow_umount_coexistence) {
        if (!store.config.disable_umount) {
            updateConfig('disable_umount', true);
        }
    }
  });

  function updateConfig<K extends keyof AppConfig>(key: K, value: AppConfig[K]) {
      store.config = { ...store.config, [key]: value };
      if (key === 'granary') {
      }
  }

  function updateGranary(key: keyof AppConfig['granary'], value: number) {
      store.config = { 
          ...store.config, 
          granary: { ...store.config.granary, [key]: value } 
      };
  }

  function save() {
    if (invalidModuleDir()) {
      store.showToast(store.L.config.invalidPath, "error");
      return;
    }
    store.saveConfig().then(() => {
        setInitialConfigStr(JSON.stringify(store.config));
    });
  }

  function reload() {
    store.loadConfig().then(() => {
        setInitialConfigStr(JSON.stringify(store.config));
    });
  }
  
  function reset() {
    setShowResetConfirm(false);
    store.resetConfig().then(() => {
        setInitialConfigStr(JSON.stringify(store.config));
    });
  }

  function toggle(key: keyof AppConfig) {
    // @ts-ignore
    const currentVal = store.config[key] as boolean;
    const newVal = !currentVal;

    if (key === 'disable_umount') {
       if (store.systemInfo?.zygisksuEnforce && store.systemInfo.zygisksuEnforce !== '0' && !store.config.allow_umount_coexistence) {
          store.showToast(store.L.config?.coexistenceRequired || "Coexistence required", "error");
          return;
       }
    }
    
    updateConfig(key, newVal);
    
    API.saveConfig({ ...store.config, [key]: newVal }).catch(() => {
        updateConfig(key, currentVal);
        store.showToast(store.L.config?.saveFailed || "Failed to update setting", "error");
    });
  }

  function setOverlayMode(mode: string) {
    updateConfig('overlay_mode', mode as OverlayMode);
  }

  const availableModes = createMemo(() => 
    store.systemInfo?.supported_overlay_modes ?? (['tmpfs', 'ext4', 'erofs'] as OverlayMode[])
  );

  const MODE_DESCS: Record<OverlayMode, string> = {
    'tmpfs': 'RAM-based. Fastest I/O, reset on reboot.',
    'ext4': 'Loopback image. Persistent, saves RAM.',
    'erofs': 'Read-only compressed. High performance, space saving.'
  };

  return (
    <>
      <md-dialog 
        open={showResetConfirm()} 
        onclose={() => setShowResetConfirm(false)}
        class="transparent-scrim"
      >
        <div slot="headline">{store.L.config?.resetConfigTitle ?? 'Reset Configuration?'}</div>
        <div slot="content">
          {store.L.config?.resetConfigConfirm ?? 'This will reset all backend settings to defaults. Continue?'}
        </div>
        <div slot="actions">
          <md-text-button onClick={() => setShowResetConfirm(false)}>
            {store.L.common?.cancel ?? 'Cancel'}
          </md-text-button>
          <md-text-button onClick={reset}>
            {store.L.config?.resetConfig ?? 'Reset Config'}
          </md-text-button>
        </div>
      </md-dialog>

      <div class="config-container">
        <section class="config-group">
          <div class="config-card">
            <div class="card-header">
              <div class="card-icon">
                <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.modules} /></svg></md-icon>
              </div>
              <div class="card-text">
                <span class="card-title">{store.L.config.moduleDir}</span>
                <span class="card-desc">{store.L.config?.moduleDirDesc ?? 'Set the directory where modules are stored'}</span>
              </div>
            </div>
            
            <div class="input-stack">
              <md-outlined-text-field 
                label={store.L.config.moduleDir} 
                value={store.config.moduledir}
                onInput={(e: any) => updateConfig('moduledir', e.currentTarget.value)}
                error={invalidModuleDir()}
                supporting-text={invalidModuleDir() ? (store.L.config?.invalidModuleDir || "Invalid Path") : ""}
                class="full-width-field"
              >
                <md-icon slot="leading-icon"><svg viewBox="0 0 24 24"><path d={ICONS.modules} /></svg></md-icon>
              </md-outlined-text-field>
            </div>
          </div>

          <div class="config-card">
            <div class="card-header">
              <div class="card-icon">
                <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.ksu} /></svg></md-icon>
              </div>
              <div class="card-text">
                <span class="card-title">{store.L.config.mountSource}</span>
                <span class="card-desc">{store.L.config?.mountSourceDesc ?? 'Global mount source namespace (e.g. KSU)'}</span>
              </div>
            </div>
            
            <div class="input-stack">
              <md-outlined-text-field 
                label={store.L.config.mountSource} 
                value={store.config.mountsource}
                onInput={(e: any) => updateConfig('mountsource', e.currentTarget.value)}
                class="full-width-field"
              >
                <md-icon slot="leading-icon"><svg viewBox="0 0 24 24"><path d={ICONS.ksu} /></svg></md-icon>
              </md-outlined-text-field>
            </div>
          </div>

          <div class="config-card">
            <div class="card-header">
              <div class="card-icon">
                <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.mount_path} /></svg></md-icon>
              </div>
              <div class="card-text">
                <span class="card-title">{store.L.config?.hybrid_mnt_dir ?? 'Mount Point Path'}</span>
                <span class="card-desc">{store.L.config?.hybrid_mnt_dir_desc ?? 'Temporary directory for OverlayFS mounting'}</span>
              </div>
            </div>
            
            <div class="input-stack">
              <md-outlined-text-field 
                label={store.L.config?.hybrid_mnt_dir ?? 'Mount Point Path'} 
                value={store.config.hybrid_mnt_dir ?? ''} 
                onInput={(e: any) => updateConfig('hybrid_mnt_dir', e.currentTarget.value)}
                class="full-width-field"
              >
                <md-icon slot="leading-icon"><svg viewBox="0 0 24 24"><path d={ICONS.mount_path} /></svg></md-icon>
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
                <span class="card-desc">{store.L.config?.partitionsDesc ?? 'Add partitions to mount'}</span>
              </div>
            </div>
            <div class="p-input">
              <ChipInput 
                values={store.config.partitions} 
                placeholder="e.g. product, system_ext..." 
                onValuesChange={(vals) => updateConfig('partitions', vals)}
              />
            </div>
          </div>
        </section>

        <Show when={store.config.granary}>
          <section class="config-group">
            <div class="config-card">
                <div class="card-header">
                    <div class="card-icon">
                        <md-icon><svg viewBox="0 0 24 24"><path d="M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2M12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4M12,6A6,6 0 0,0 6,12A6,6 0 0,0 12,18A6,6 0 0,0 18,12A6,6 0 0,0 12,6M12,8A4,4 0 0,1 16,12A4,4 0 0,1 12,16A4,4 0 0,1 8,12A4,4 0 0,1 12,8Z" /></svg></md-icon>
                    </div>
                    <div class="card-text">
                        <span class="card-title">{store.L.config?.granarySettings || "Granary Settings"}</span>
                    </div>
                </div>

                <div class="setting-list">
                    <div class="list-item">
                        <div class="list-text">
                            <span class="list-title">{store.L.config?.maxBackups || "Max Backups"}</span>
                            <span class="list-desc">{store.L.config?.maxBackupsDesc || "Max snapshots to keep"}</span>
                        </div>
                        <input 
                            type="number" 
                            class="num-input" 
                            min="1" max="100"
                            aria-label={store.L.config?.maxBackups || "Max Backups"} 
                            value={store.config.granary.max_backups}
                            onInput={(e) => updateGranary('max_backups', parseInt(e.currentTarget.value) || 0)}
                        />
                    </div>
                    
                    <div class="item-separator"></div>

                    <div class="list-item">
                        <div class="list-text">
                            <span class="list-title">{store.L.config?.retentionDays || "Retention Days"}</span>
                            <span class="list-desc">{store.L.config?.retentionDaysDesc || "0 = Unlimited"}</span>
                        </div>
                        <input 
                            type="number" 
                            class="num-input" 
                            min="0" max="365"
                            aria-label={store.L.config?.retentionDays || "Retention Days"}
                            value={store.config.granary.retention_days}
                            onInput={(e) => updateGranary('retention_days', parseInt(e.currentTarget.value) || 0)}
                        />
                    </div>
                </div>
            </div>
          </section>
        </Show>

        <section class="config-group">
          <div class="config-card">
            <div class="card-header">
              <div class="card-icon">
                <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.save} /></svg></md-icon>
              </div>
              <div class="card-text">
                <span class="card-title">{store.L.config?.overlayMode || "Overlay Mode"}</span>
                <span class="card-desc">{store.L.config?.overlayModeDesc || "Select backing storage strategy"}</span>
              </div>
            </div>
            <div class="mode-selector">
              <For each={availableModes()}>
                {(mode) => (
                  <button 
                    class={`mode-item ${store.config.overlay_mode === mode ? 'selected' : ''}`}
                    onClick={() => setOverlayMode(mode)}
                  >
                    <md-ripple></md-ripple>
                    <div class="mode-info">
                      <span class="mode-title">{store.L.config?.[`mode_${mode}`] || mode}</span>
                      <span class="mode-desc">{store.L.config?.[`mode_${mode}Desc`] || MODE_DESCS[mode]}</span>
                    </div>
                    <div class="mode-check">
                       <md-icon><svg viewBox="0 0 24 24"><path d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z" /></svg></md-icon>
                    </div>
                  </button>
                )}
              </For>
            </div>
          </div>

          <div class="options-grid">
            <button 
              class={`option-tile clickable error ${store.config.enable_nuke ? 'active' : ''}`}
              onClick={() => toggle('enable_nuke')}
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
              class={`option-tile clickable tertiary ${store.config.disable_umount ? 'active' : ''}`}
              onClick={() => toggle('disable_umount')}
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

            
            <Show when={store.systemInfo?.zygisksuEnforce && store.systemInfo.zygisksuEnforce !== '0'}>
              <button 
                class={`option-tile clickable error ${store.config.allow_umount_coexistence ? 'active' : ''}`}
                onClick={() => toggle('allow_umount_coexistence')}
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
            </Show>

            <button 
              class={`option-tile clickable primary ${store.config.verbose ? 'active' : ''}`}
              onClick={() => toggle('verbose')}
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

            <Show when={store.config.verbose}>
              <button 
                class={`option-tile clickable secondary ${store.config.dry_run ? 'active' : ''}`}
                onClick={() => toggle('dry_run')}
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
            </Show>
          </div>
        </section>

        <section class="config-group">
          <div class="webui-label">
              {store.L.config?.webui || 'WebUI'}
          </div>
          <div class="options-grid">
            <button 
              class={`option-tile clickable secondary ${store.fixBottomNav ? 'active' : ''}`}
              onClick={store.toggleBottomNavFix}
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

            <button 
              class="option-tile clickable error" 
              onClick={() => setShowResetConfirm(true)}
              disabled={store.saving.config}
            >
              <md-ripple></md-ripple>
              <div class="tile-top">
                <div class="tile-icon">
                  <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.replay} /></svg></md-icon>
                </div>
              </div>
              <div class="tile-bottom">
                <span class="tile-label">{store.L.config?.resetConfig || 'Reset Config'}</span>
              </div>
            </button>
          </div>
        </section>
      </div>

      <BottomActions>
        <md-filled-tonal-icon-button 
          onClick={reload}
          disabled={store.loading.config}
          title={store.L.config.reload}
          role="button"
          tabIndex={0}
        >
          <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.refresh} /></svg></md-icon>
        </md-filled-tonal-icon-button>
        
        <div class="spacer"></div>

        <md-filled-button 
          onClick={save} 
          disabled={store.saving.config || !isDirty()}
          role="button"
          tabIndex={0}
        >
          <md-icon slot="icon"><svg viewBox="0 0 24 24"><path d={ICONS.save} /></svg></md-icon>
          {store.saving.config ? store.L.common.saving : store.L.config.save}
        </md-filled-button>
      </BottomActions>
    </>
  );
}