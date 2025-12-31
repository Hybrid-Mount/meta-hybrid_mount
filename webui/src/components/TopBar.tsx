/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, onMount, onCleanup, Show, For } from 'solid-js';
import { store } from '../lib/store';
import { ICONS } from '../lib/constants';
import './TopBar.css';
import '@material/web/icon/icon.js';
import '@material/web/iconbutton/icon-button.js';

export default function TopBar() {
  const [showLangMenu, setShowLangMenu] = createSignal(false);
  let langButtonRef: HTMLElement | undefined;
  let menuRef: HTMLDivElement | undefined;

  function toggleTheme() {
    let nextTheme: 'light' | 'dark' | 'auto';
    let toastMsg: string;
    const common = store.L?.common;
    const currentTheme = store.theme;

    if (currentTheme === 'auto') {
      nextTheme = 'light';
      toastMsg = common?.themeLight ?? 'Light Mode';
    } else if (currentTheme === 'light') {
      nextTheme = 'dark';
      toastMsg = common?.themeDark ?? 'Dark Mode';
    } else {
      nextTheme = 'auto';
      toastMsg = common?.themeAuto ?? 'Auto Mode';
    }
    store.setTheme(nextTheme);
    store.showToast(toastMsg, 'info');
  }

  function getThemeIcon() {
    if (store.theme === 'auto') return ICONS.auto_mode;
    if (store.theme === 'light') return ICONS.light_mode;
    return ICONS.dark_mode;
  }

  function setLang(code: string) {
    store.setLang(code);
    setShowLangMenu(false);
  }

  function handleOutsideClick(e: MouseEvent) {
    if (
      showLangMenu() && 
      menuRef && !menuRef.contains(e.target as Node) && 
      langButtonRef && !langButtonRef.contains(e.target as Node)
    ) {
      setShowLangMenu(false);
    }
  }

  onMount(() => {
    window.addEventListener('click', handleOutsideClick);
  });

  onCleanup(() => {
    window.removeEventListener('click', handleOutsideClick);
  });

  return (
    <header class="top-bar">
      <div class="top-bar-content">
        <h1 class="screen-title">{store.L?.common?.appName}</h1>
        <div class="top-actions">
          <md-icon-button 
            onClick={toggleTheme} 
            title={store.L?.common?.theme}
            role="button"
            tabIndex={0}
          >
            <md-icon>
              <svg viewBox="0 0 24 24"><path d={getThemeIcon()} /></svg>
            </md-icon>
          </md-icon-button>

          <div style={{ position: 'relative', display: 'inline-flex' }}>
            <md-icon-button 
              ref={langButtonRef}
              onClick={() => setShowLangMenu(!showLangMenu())} 
              title={store.L?.common?.language}
              role="button"
              tabIndex={0}
            >
              <md-icon>
                <svg viewBox="0 0 24 24"><path d={ICONS.translate} /></svg>
              </md-icon>
            </md-icon-button>

            <Show when={showLangMenu()}>
              <div class="menu-dropdown" ref={menuRef}>
                <For fallback={[]} each={store.availableLanguages}>
                  {(l) => (
                    <button class="menu-item" onClick={() => setLang(l.code)}>{l.name}</button>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </header>
  );
}