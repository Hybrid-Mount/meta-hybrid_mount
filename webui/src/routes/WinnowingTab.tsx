/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createMemo, onMount, Show, For } from 'solid-js';
import { API } from '../lib/api';
import { store } from '../lib/store';
import { ICONS } from '../lib/constants';
import type { ConflictEntry } from '../lib/types';
import Skeleton from '../components/Skeleton';
import BottomActions from '../components/BottomActions';
import './WinnowingTab.css';

import '@material/web/textfield/outlined-text-field.js';
import '@material/web/chips/chip-set.js';
import '@material/web/chips/filter-chip.js';
import '@material/web/icon/icon.js';
import '@material/web/divider/divider.js';
import '@material/web/iconbutton/filled-tonal-icon-button.js';

export default function WinnowingTab() {
  const [conflicts, setConflicts] = createSignal<ConflictEntry[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [searchTerm, setSearchTerm] = createSignal("");

  const L_W = createMemo(() => store.L.winnowing || {});
  const L_C = createMemo(() => store.L.common || {});

  function getFullPath(entry: ConflictEntry) {
    return `/${entry.partition}/${entry.relative_path}`;
  }

  async function loadData() {
    setLoading(true);
    try {
      const data = await API.getConflicts();
      setConflicts(data);
      if (store.conflicts.length !== data.length) {
          store.loadConflicts();
      }
    } catch (e) {
      store.showToast(store.L.modules?.conflictError || "Failed to load", "error");
    } finally {
      setLoading(false);
    }
  }

  async function selectWinner(item: ConflictEntry, moduleId: string) {
    const list = conflicts();
    const idx = list.findIndex(c => c.partition === item.partition && c.relative_path === item.relative_path);
    if (idx !== -1) {
      const newList = [...list];
      newList[idx] = { ...newList[idx], selected: moduleId, is_forced: true };
      setConflicts(newList);
    }
    try {
      await API.setWinnowingRule(getFullPath(item), moduleId);
    } catch(e) {
      store.showToast("Failed to set rule", "error");
    }
  }

  const filteredConflicts = createMemo(() => conflicts().filter(c => 
    getFullPath(c).toLowerCase().includes(searchTerm().toLowerCase())
  ));

  onMount(loadData);

  return (
    <>
      <div class="winnow-page">
        <div class="search-box">
            <md-outlined-text-field
              placeholder={L_W().searchPlaceholder || "Search paths..."}
              value={searchTerm()}
              onInput={(e: any) => setSearchTerm(e.currentTarget.value)}
              class="full-width-field"
            >
              <md-icon slot="leading-icon"><svg viewBox="0 0 24 24"><path d={ICONS.search} /></svg></md-icon>
            </md-outlined-text-field>
        </div>

        <Show when={!loading()} fallback={
          <div class="list-wrapper">
              <For each={Array(4)}>{() =>
                  <div class="conflict-card skeleton-card">
                      <Skeleton width="100%" height="100px" borderRadius="16px"/>
                  </div>
              }</For>
          </div>
        }>
          <Show when={conflicts().length > 0} fallback={
            <div class="clean-state">
              <div class="clean-icon-circle">
                  <md-icon class="clean-icon"><svg viewBox="0 0 24 24"><path d={ICONS.check} /></svg></md-icon>
              </div>
              <h3>{L_W().emptyTitle || 'All Clear'}</h3>
              <p>{L_W().emptyDesc || 'No conflicts detected.'}</p>
            </div>
          }>
            <div class="conflict-list">
              <For each={filteredConflicts()}>
                {(item) => (
                  <div class={`conflict-card ${item.is_forced ? 'forced' : ''}`}>
                    <div class="card-header">
                      <md-icon class="file-icon"><svg viewBox="0 0 24 24"><path d={ICONS.description} /></svg></md-icon>
                      <div class="path-info">
                          <span class="path-label">{L_W().conflictPath || 'Conflict Path'}</span>
                          <span class="path-text" title={getFullPath(item)}>{getFullPath(item)}</span>
                      </div>
                    </div>
                    
                    <md-divider></md-divider>

                    <div class="card-body">
                        <span class="selection-label">{L_W().selectProvider || 'Select Provider'}:</span>
                        <md-chip-set>
                          <For each={item.contending_modules}>
                            {(modId) => (
                              <md-filter-chip 
                                label={modId}
                                selected={item.selected === modId}
                                onClick={() => selectWinner(item, modId)}
                              ></md-filter-chip>
                            )}
                          </For>
                        </md-chip-set>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </Show>
      </div>

      <BottomActions>
          <div class="spacer"></div>
          <md-filled-tonal-icon-button 
            onClick={loadData} 
            disabled={loading()}
            title={L_C().refresh || "Refresh"}
          >
            <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.refresh} /></svg></md-icon>
          </md-filled-tonal-icon-button>
      </BottomActions>
    </>
  );
}