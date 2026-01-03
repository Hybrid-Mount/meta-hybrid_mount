/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createMemo, onMount, Show, For } from 'solid-js';
import { store } from '../lib/store';
import { ICONS } from '../lib/constants';
import Skeleton from '../components/Skeleton';
import BottomActions from '../components/BottomActions';
import { API } from '../lib/api';
import type { Module, MountMode } from '../lib/types';
import './ModulesTab.css';
import '@material/web/iconbutton/filled-tonal-icon-button.js';
import '@material/web/button/filled-button.js';
import '@material/web/icon/icon.js';

declare module "solid-js" {
  namespace JSX {
    interface Directives {
      style?: any;
    }
  }
}

export default function ModulesTab() {
  const [searchQuery, setSearchQuery] = createSignal('');
  const [filterType, setFilterType] = createSignal('all');
  const [showUnmounted, setShowUnmounted] = createSignal(false);
  const [expandedId, setExpandedId] = createSignal<string | null>(null);
  const [initialRulesSnapshot, setInitialRulesSnapshot] = createSignal<Record<string, string>>({});
  const [showConflicts, setShowConflicts] = createSignal(false);

  onMount(() => {
    load();
  });

  function load() {
    store.loadModules().then(() => {
        const snapshot: Record<string, string> = {};
        store.modules.forEach(m => {
            snapshot[m.id] = JSON.stringify(m.rules);
        });
        setInitialRulesSnapshot(snapshot);
    });
  }

  const dirtyModules = createMemo(() => store.modules.filter(m => {
      const initial = initialRulesSnapshot()[m.id];
      if (!initial) return false;
      return JSON.stringify(m.rules) !== initial;
  }));

  const isDirty = createMemo(() => dirtyModules().length > 0);
  function updateModule(modId: string, transform: (m: Module) => Module) {
      const idx = store.modules.findIndex(m => m.id === modId);
      if (idx === -1) return;
      
      const newModules = [...store.modules];
      newModules[idx] = transform({ ...newModules[idx] }); 
      store.modules = newModules;
  }

  const [isSaving, setIsSaving] = createSignal(false);

  async function performSave() {
    setIsSaving(true);
    try {
        const dirty = dirtyModules();
        for (const mod of dirty) {
            await API.saveModuleRules(mod.id, mod.rules);
        }
        await load();
        store.showToast(store.L.modules?.saveSuccess || store.L.common?.saveSuccess || "Saved successfully", 'success');
    } catch (e: any) {
        console.error(e);
        store.showToast(e.message || store.L.modules?.saveFailed || "Failed to save", 'error');
    } finally {
        setIsSaving(false);
    }
  }

  const filteredModules = createMemo(() => store.modules.filter(m => {
    const q = searchQuery().toLowerCase();
    const matchSearch = m.name.toLowerCase().includes(q) || m.id.toLowerCase().includes(q);
    const matchFilter = filterType() === 'all' || m.mode === filterType();
    const matchMounted = showUnmounted() || m.is_mounted;
    return matchSearch && matchFilter && matchMounted;
  }));

  function toggleExpand(id: string) {
    if (expandedId() === id) {
      setExpandedId(null);
    } else {
      setExpandedId(id);
      setShowConflicts(false);
    }
  }

  function handleKeydown(e: KeyboardEvent, id: string) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      toggleExpand(id);
    }
  }

  function getModeLabel(mod: Module) {
      const m = store.L.modules?.modes;
      if (!mod.is_mounted) return m?.none ?? 'None';
      
      const mode = mod.rules.default_mode;
      if (mode === 'magic') return m?.magic ?? 'Magic Mount';
      if (mode === 'ignore') return m?.ignore ?? 'Ignore';
      return m?.auto ?? 'OverlayFS';
  }

  function updateModuleRules(modId: string, updateFn: (rules: Module['rules']) => Module['rules']) {
      updateModule(modId, m => ({ ...m, rules: updateFn(m.rules) }));
  }

  function addPathRule(mod: Module) {
      updateModuleRules(mod.id, rules => {
        const paths = rules.paths ? { ...rules.paths } : {};
        let newKey = "new/path";
        let counter = 1;
        while (newKey in paths) {
            newKey = `new/path${counter++}`;
        }
        paths[newKey] = 'magic';
        return { ...rules, paths };
      });
  }

  function removePathRule(mod: Module, path: string) {
      updateModuleRules(mod.id, rules => {
          const paths = { ...rules.paths };
          delete paths[path];
          return { ...rules, paths };
      });
  }

  function updatePathKey(mod: Module, oldPath: string, newPath: string) {
      if (oldPath === newPath || !newPath.trim()) return;
      updateModuleRules(mod.id, rules => {
          const paths = { ...rules.paths };
          const mode = paths[oldPath];
          delete paths[oldPath];
          paths[newPath] = mode;
          return { ...rules, paths };
      });
  }

  function updatePathMode(mod: Module, path: string, mode: MountMode) {
      updateModuleRules(mod.id, rules => {
          const paths = { ...rules.paths };
          paths[path] = mode;
          return { ...rules, paths };
      });
  }

  function updateDefaultMode(mod: Module, mode: MountMode) {
      updateModuleRules(mod.id, rules => ({ ...rules, default_mode: mode }));
  }

  async function checkConflicts() {
      if (showConflicts()) {
          setShowConflicts(false);
      } else {
          setShowConflicts(true);
          setExpandedId(null);
          if (store.conflicts.length === 0) {
              await store.loadConflicts();
          }
      }
  }

  return (
    <>
      <div class="header-wrapper">
          <div class="md3-card desc-card">
            <p class="desc-text mb-12">
              {store.L.modules?.desc}
            </p>
            <button class={`btn-tonal conflict-btn ${showConflicts() ? 'active' : ''}`} onClick={checkConflicts}>
              {showConflicts() ? (store.L.modules?.hideConflicts || 'Hide Conflicts') : (store.L.modules?.checkConflicts || 'Check Conflicts')}
            </button>
          </div>

          <Show when={showConflicts()}>
              <div class="md3-card conflict-panel">
                  <div class="conflict-header-row">
                      <div class="conflict-title">
                          <svg viewBox="0 0 24 24" width="20" height="20" class="conflict-icon"><path d={ICONS.warning} fill="currentColor"/></svg>
                          {store.L.modules?.conflictsTitle || 'File Conflicts'}
                      </div>
                      <button class="btn-icon-small" onClick={() => setShowConflicts(false)} title="Close">
                          <svg viewBox="0 0 24 24" width="18" height="18"><path d={ICONS.close} fill="currentColor"/></svg>
                      </button>
                  </div>

                  <Show when={!store.loading.conflicts} fallback={
                      <div class="skeleton-group">
                          <Skeleton width="100%" height="40px" />
                          <Skeleton width="100%" height="40px" />
                          <Skeleton width="80%" height="40px" />
                      </div>
                  }>
                      <Show when={store.conflicts.length > 0} fallback={
                          <div class="conflict-empty">
                              <svg viewBox="0 0 24 24" width="48" height="48" class="conflict-empty-icon"><path d={ICONS.check} fill="currentColor"/></svg>
                              <div>{store.L.modules?.noConflicts || 'No file conflicts detected.'}</div>
                          </div>
                      }>
                          <div class="conflict-list">
                              <For each={store.conflicts}>
                                {(conflict) => (
                                  <div class="conflict-item">
                                      <div class="conflict-path">
                                          /{conflict.partition}/{conflict.relative_path}
                                      </div>
                                      <div class="conflict-modules">
                                          <For each={conflict.contending_modules}>
                                            {(modName) => <span class="module-capsule">{modName}</span>}
                                          </For>
                                      </div>
                                  </div>
                                )}
                              </For>
                          </div>
                      </Show>
                  </Show>
              </div>
          </Show>
      </div>

      <div class="search-container">
        <svg class="search-icon" viewBox="0 0 24 24"><path d={ICONS.search} /></svg>
        <input 
          type="text" 
          class="search-input" 
          placeholder={store.L.modules?.searchPlaceholder}
          value={searchQuery()}
          onInput={(e) => setSearchQuery(e.currentTarget.value)}
        />
        <div class="filter-controls">
          <div class="checkbox-wrapper">
              <input 
                type="checkbox" 
                id="show-unmounted" 
                checked={showUnmounted()} 
                onChange={(e) => setShowUnmounted(e.currentTarget.checked)}
              />
              <label for="show-unmounted" title="Show unmounted modules">{store.L.modules?.filterAll ?? 'All'}</label>
          </div>
          <div class="vertical-divider"></div>
          <span class="filter-label-text">{store.L.modules?.filterLabel}</span>
          <select 
            class="filter-select" 
            value={filterType()} 
            onChange={(e) => setFilterType(e.currentTarget.value)}
            aria-label="Filter Modules"
          >
            <option value="all">{store.L.modules?.filterAll}</option>
            <option value="auto">{store.L.modules?.modeAuto}</option>
            <option value="magic">{store.L.modules?.modeMagic}</option>
          </select>
        </div>
      </div>

      <Show when={!store.loading.modules} fallback={
        <div class="rules-list">
          <For each={Array(5)}>{() => 
            <div class="rule-card">
              <div class="rule-info">
                <div class="skeleton-group">
                  <Skeleton width="60%" height="20px" />
                  <Skeleton width="40%" height="14px" />
                </div>
              </div>
              <Skeleton width="120px" height="40px" borderRadius="4px" />
            </div>
          }</For>
        </div>
      }>
        <Show when={filteredModules().length > 0} fallback={
          <div class="empty-state">
            {store.modules.length === 0 ? (store.L.modules?.empty ?? "No enabled modules found") : "No matching modules"}
          </div>
        }>
          <div class="rules-list">
            <For each={filteredModules()}>
              {(mod, i) => (
                <div 
                  class={`rule-card ${expandedId() === mod.id ? 'expanded' : ''} ${initialRulesSnapshot()[mod.id] !== JSON.stringify(mod.rules) ? 'dirty' : ''} ${!mod.is_mounted ? 'unmounted' : ''}`}
                  style={{ "--i": i() }}
                >
                  <div 
                      class="rule-main"
                      onClick={() => toggleExpand(mod.id)}
                      onKeyDown={(e) => handleKeydown(e, mod.id)}
                      role="button"
                      tabIndex={0}
                  >
                    <div class="rule-info">
                      <div class="info-col">
                        <span class="module-name">{mod.name}</span>
                        <span class="module-id">{mod.id} <span class="version-tag">{mod.version}</span></span>
                      </div>
                    </div>
                    <div 
                      class={`mode-badge ${!mod.is_mounted ? 'badge-none' : mod.rules.default_mode === 'magic' ? 'badge-magic' : 'badge-auto'}`}
                    >
                      {getModeLabel(mod)}
                    </div>
                  </div>
                  
                  <div class={`rule-details-wrapper ${expandedId() === mod.id ? 'open' : ''}`}>
                    <div class="rule-details-inner">
                      <div class="rule-details">
                        <p class="module-desc">{mod.description || (store.L.modules?.noDesc ?? 'No description')}</p>
                        <p class="module-meta">{store.L.modules?.author ?? 'Author'}: {mod.author || (store.L.modules?.unknown ?? 'Unknown')}</p>
                        
                        <Show when={!mod.is_mounted}>
                              <div class="status-alert">
                                <svg viewBox="0 0 24 24" width="16" height="16"><path d={ICONS.info} fill="currentColor"/></svg>
                                <span>This module is currently not mounted.</span>
                            </div>
                        </Show>
                  
                        <div class="config-section">
                          <div class="config-row">
                            <span class="config-label">{store.L.modules?.defaultMode ?? 'Default Strategy'}:</span>
                            <div class="text-field compact-select">
                              <select 
                                value={mod.rules.default_mode}
                                onChange={(e) => updateDefaultMode(mod, e.currentTarget.value as MountMode)}
                                onClick={(e) => e.stopPropagation()}
                                aria-label="Default Strategy"
                              >
                                <option value="overlay">{store.L.modules?.modes?.auto ?? 'OverlayFS (Auto)'}</option>
                                <option value="magic">{store.L.modules?.modes?.magic ?? 'Magic Mount'}</option>
                                <option value="ignore">{store.L.modules?.modes?.ignore ?? 'Disable (Ignore)'}</option>
                              </select>
                            </div>
                          </div>

                          <div class="paths-editor">
                            <div class="paths-header">
                                <span class="config-label">{store.L.modules?.pathRules ?? 'Path Overrides'}:</span>
                                <button class="btn-icon add-rule" onClick={() => addPathRule(mod)} title={store.L.modules?.addRule ?? 'Add Rule'}>
                                    <svg viewBox="0 0 24 24" width="20" height="20"><path d={ICONS.add} fill="currentColor"/></svg>
                                </button>
                            </div>
                            
                            <Show when={mod.rules.paths && Object.keys(mod.rules.paths).length > 0} fallback={
                                <div class="empty-paths">{store.L.modules?.noRules ?? 'No path overrides defined.'}</div>
                            }>
                                <div class="path-list">
                                    <For each={Object.entries(mod.rules.paths)}>
                                      {([path, mode]) => (
                                        <div class="path-row">
                                            <input 
                                                type="text" 
                                                class="path-input" 
                                                value={path} 
                                                onChange={(e) => updatePathKey(mod, path, e.currentTarget.value)}
                                                placeholder={store.L.modules?.placeholder ?? "e.g. system/fonts"}
                                            />
                                            <select 
                                                class="path-mode-select"
                                                value={mode}
                                                onChange={(e) => updatePathMode(mod, path, e.currentTarget.value as MountMode)}
                                                aria-label="Path Mode"
                                            >
                                                <option value="overlay">{store.L.modules?.modes?.short?.auto ?? 'Overlay'}</option>
                                                <option value="magic">{store.L.modules?.modes?.short?.magic ?? 'Magic'}</option>
                                                <option value="ignore">{store.L.modules?.modes?.short?.ignore ?? 'Ignore'}</option>
                                            </select>
                                            <button class="btn-icon delete" onClick={() => removePathRule(mod, path)} title="Remove rule">
                                                <svg viewBox="0 0 24 24" width="18" height="18"><path d={ICONS.delete} fill="currentColor"/></svg>
                                            </button>
                                        </div>
                                      )}
                                    </For>
                                </div>
                            </Show>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>

      <BottomActions>
        <md-filled-tonal-icon-button 
          onClick={load} 
          disabled={store.loading.modules}
          title={store.L.modules?.reload}
          role="button"
          tabIndex={0}
        >
          <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.refresh} /></svg></md-icon>
        </md-filled-tonal-icon-button>

        <div class="spacer"></div>
       
        <md-filled-button 
          onClick={performSave} 
          disabled={isSaving() || !isDirty()}
          role="button"
          tabIndex={0}
        >
          <md-icon slot="icon"><svg viewBox="0 0 24 24"><path d={ICONS.save} /></svg></md-icon>
          {isSaving() ? store.L.common?.saving : store.L.modules?.save}
        </md-filled-button>
      </BottomActions>
    </>
  );
}