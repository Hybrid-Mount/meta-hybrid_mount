/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createMemo, createEffect, onMount, onCleanup, Show, For } from 'solid-js';
import { store } from '../lib/store';
import { ICONS } from '../lib/constants';
import Skeleton from '../components/Skeleton';
import BottomActions from '../components/BottomActions';
import './LogsTab.css';
import '@material/web/iconbutton/filled-tonal-icon-button.js';
import '@material/web/icon/icon.js';

export default function LogsTab() {
  const [searchLogQuery, setSearchLogQuery] = createSignal('');
  const [filterLevel, setFilterLevel] = createSignal('all');
  const [autoRefresh, setAutoRefresh] = createSignal(false);
  const [userHasScrolledUp, setUserHasScrolledUp] = createSignal(false);
  
  let logContainer: HTMLDivElement | undefined;
  let refreshInterval: number | undefined;

  const filteredLogs = createMemo(() => store.logs.filter(line => {
    const text = line.text.toLowerCase();
    const matchesSearch = text.includes(searchLogQuery().toLowerCase());
    let matchesLevel = true;
    if (filterLevel() !== 'all') {
      matchesLevel = line.type === filterLevel();
    }
    return matchesSearch && matchesLevel;
  }));

  async function scrollToBottom() {
    if (logContainer) { 
      logContainer.scrollTo({ top: logContainer.scrollHeight, behavior: 'smooth' });
      setUserHasScrolledUp(false);
    }
  }

  function handleScroll(e: Event) {
    const target = e.target as HTMLElement;
    const { scrollTop, scrollHeight, clientHeight } = target;
    const distanceToBottom = scrollHeight - scrollTop - clientHeight;
    setUserHasScrolledUp(distanceToBottom > 50);
  }

  async function refreshLogs(silent = false) {
    await store.loadLogs(silent);
    if (!silent && !userHasScrolledUp()) {
      if (logContainer) {
        logContainer.scrollTop = logContainer.scrollHeight;
      }
    }
  }

  async function copyLogs() {
    const logs = filteredLogs();
    if (logs.length === 0) return;
    const text = logs.map(l => l.text).join('\n');
    try {
      await navigator.clipboard.writeText(text);
      store.showToast(store.L.logs.copySuccess, 'success');
    } catch (e) {
      store.showToast(store.L.logs.copyFail, 'error');
    }
  }

  createEffect(() => {
    if (autoRefresh()) {
      refreshLogs(true); 
      refreshInterval = window.setInterval(() => {
        refreshLogs(true); 
      }, 3000);
    } else {
      if (refreshInterval) clearInterval(refreshInterval);
    }
  });

  onMount(() => {
    refreshLogs(); 
  });

  onCleanup(() => {
    if (refreshInterval) clearInterval(refreshInterval);
  });

  return (
    <>
      <div class="logs-controls">
        <svg viewBox="0 0 24 24" width="20" height="20" class="log-search-icon">
          <path d={ICONS.search} />
        </svg>
        <input 
          type="text" 
          class="log-search-input" 
          placeholder={store.L.logs.searchPlaceholder}
          value={searchLogQuery()}
          onInput={(e) => setSearchLogQuery(e.currentTarget.value)}
        />
        <div class="log-auto-group">
          <input 
            type="checkbox" 
            id="auto-refresh" 
            checked={autoRefresh()} 
            onChange={(e) => setAutoRefresh(e.currentTarget.checked)}
            class="log-auto-checkbox" 
          />
          <label for="auto-refresh" class="log-auto-label">Auto</label>
        </div>
        <div class="log-divider"></div>
        <span class="log-filter-label">
          {store.L.logs.filterLabel}
        </span>
        <select 
          class="log-filter-select" 
          value={filterLevel()} 
          onChange={(e) => setFilterLevel(e.currentTarget.value)}
          aria-label={store.L.logs.filterLabel || "Filter Level"}
        >
          <option value="all">{store.L.logs.levels.all}</option>
          <option value="info">{store.L.logs.levels.info}</option>
          <option value="warn">{store.L.logs.levels.warn}</option>
          <option value="error">{store.L.logs.levels.error}</option>
        </select>
      </div>

      <div class="log-container" ref={logContainer} onScroll={handleScroll}>
        <Show when={!store.loading.logs} fallback={
          <div class="log-skeleton-container">
            <For each={Array(10)}>{(_, i) => <Skeleton width={`${60 + (i() % 3) * 20}%`} height="14px" />}</For>
          </div>
        }>
          <Show when={filteredLogs().length > 0} fallback={
             <div class="log-empty-state">
               {store.logs.length === 0 ? store.L.logs.empty : "No matching logs"}
             </div>
          }>
             <For each={filteredLogs()}>
               {(line) => (
                 <span class="log-entry">
                   <span class={`log-${line.type}`}>{line.text}</span>
                 </span>
               )}
             </For>
             <div class="log-footer">
               — Showing last 1000 lines —
             </div>
          </Show>
        </Show>
        
        <Show when={userHasScrolledUp()}>
          <button 
            class="scroll-fab" 
            onClick={scrollToBottom}
            title="Scroll to bottom"
          >
            <svg viewBox="0 0 24 24" class="scroll-icon"><path d="M11 4h2v12l5.5-5.5 1.42 1.42L12 19.84l-7.92-7.92L5.5 10.5 11 16V4z" fill="currentColor"/></svg>
            Latest
          </button>
        </Show>
      </div>

      <BottomActions>
        <md-filled-tonal-icon-button 
          onClick={copyLogs} 
          disabled={filteredLogs().length === 0} 
          title={store.L.logs.copy}
          role="button"
          tabIndex={0}
        >
          <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.copy} /></svg></md-icon>
        </md-filled-tonal-icon-button>

        <div class="spacer"></div>

        <md-filled-tonal-icon-button 
          onClick={() => refreshLogs(false)} 
          disabled={store.loading.logs}
          title={store.L.logs.refresh}
          role="button"
          tabIndex={0}
        >
          <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.refresh} /></svg></md-icon>
        </md-filled-tonal-icon-button>
      </BottomActions>
    </>
  );
}