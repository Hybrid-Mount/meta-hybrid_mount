/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, onMount, Show, For, createMemo } from 'solid-js';
import { store } from '../lib/store';
import { API } from '../lib/api';
import { ICONS } from '../lib/constants';
import './InfoTab.css';
import Skeleton from '../components/Skeleton';
import '@material/web/button/filled-tonal-button.js';
import '@material/web/icon/icon.js';
import '@material/web/list/list.js';
import '@material/web/list/list-item.js';

const REPO_OWNER = 'YuzakiKokuban';
const REPO_NAME = 'meta-hybrid_mount';
const DONATE_LINK = `https://afdian.com/a/${REPO_OWNER}`;
const TELEGRAM_LINK = 'https://t.me/hybridmountchat';
const CACHE_KEY = 'hm_contributors_cache';
const CACHE_DURATION = 1000 * 60 * 60;

interface Contributor {
  login: string;
  avatar_url: string;
  html_url: string;
  type: string;
  url: string;
  name?: string;
  bio?: string;
}

export default function InfoTab() {
  const [contributors, setContributors] = createSignal<Contributor[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal(false);
  const [version, setVersion] = createSignal(store.version);

  const isDev = createMemo(() => {
    return !/^v\d+\.\d+\.\d+$/.test(version());
  });

  onMount(async () => {
    try {
        const v = await API.getVersion();
        if (v) setVersion(v);
    } catch (e) {
        console.error("Failed to fetch version", e);
    }
    await fetchContributors();
  });

  async function fetchContributors() {
    const cached = localStorage.getItem(CACHE_KEY);
    if (cached) {
      try {
        const { data, timestamp } = JSON.parse(cached);
        if (Date.now() - timestamp < CACHE_DURATION) {
          setContributors(data);
          setLoading(false);
          return;
        }
      } catch (e) {
        localStorage.removeItem(CACHE_KEY);
      }
    }

    try {
      const res = await fetch(`https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/contributors`);
      if (!res.ok) throw new Error('Failed to fetch list');
      
      const basicList = await res.json();
      const filteredList = basicList.filter((user: Contributor) => {
        const isBotType = user.type === 'Bot';
        const hasBotName = user.login.toLowerCase().includes('bot');
        return !isBotType && !hasBotName;
      });

      const detailPromises = filteredList.map(async (user: Contributor) => {
        try {
            const detailRes = await fetch(user.url);
            if (detailRes.ok) {
                const detail = await detailRes.json();
                return { ...user, bio: detail.bio, name: detail.name || user.login };
            }
        } catch (e) {
            console.warn('Failed to fetch detail for', user.login);
        }
        return user;
      });

      const results = await Promise.all(detailPromises);
      setContributors(results);
      localStorage.setItem(CACHE_KEY, JSON.stringify({
        data: results,
        timestamp: Date.now()
      }));
    } catch (e) {
      console.error(e);
      setError(true);
    } finally {
      setLoading(false);
    }
  }

  function handleLink(e: MouseEvent, url: string) {
    e.preventDefault();
    API.openLink(url);
  }

  return (
    <div class="info-container">
      <div class="project-header">
        <div class="app-logo">
          <Show when={!isDev()} fallback={
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120" class="dev-logo">
              <circle cx="60" cy="60" r="50" class="logo-base-track" />
              <circle cx="60" cy="60" r="38" class="logo-base-track" />
              <circle cx="60" cy="60" r="26" class="logo-base-track" />
              
              <g style={{ "transform-origin": "center", transform: "rotate(-45deg)" }}>
                <path d="M 60 10 A 50 50 0 1 1 10 60" class="logo-arc logo-arc-outer" />
              </g>

              <g style={{ "transform-origin": "center", transform: "rotate(135deg)" }}>
                 <path d="M 60 22 A 38 38 0 0 1 60 98" class="logo-arc logo-arc-mid logo-arc-error" />
              </g>
              
              <g style={{ "transform-origin": "center", transform: "rotate(270deg)" }}>
                <path d="M 60 34 A 26 26 0 1 1 47 82.5" class="logo-arc logo-arc-inner" />
              </g>
              
              <circle cx="60" cy="60" r="10" class="logo-core" />
            </svg>
          }>
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120">
              <circle cx="60" cy="60" r="50" class="logo-base-track" />
              <circle cx="60" cy="60" r="38" class="logo-base-track" />
              <circle cx="60" cy="60" r="26" class="logo-base-track" />
              
              <path d="M60 10 A 50 50 0 0 1 110 60" class="logo-arc logo-arc-outer" />
              <path d="M60 98 A 38 38 0 0 1 60 22" class="logo-arc logo-arc-mid" />
              <path d="M34 60 A 26 26 0 1 1 86 60" class="logo-arc logo-arc-inner" />
              
              <circle cx="60" cy="60" r="10" class="logo-core" />
            </svg>
          </Show>
        </div>
        <span class="app-name">{store.L.common.appName}</span>
        <span class="app-version">{version()}</span>
      </div>

      <div class="action-buttons">
        <md-filled-tonal-button 
           class="action-btn"
           onClick={(e: MouseEvent) => handleLink(e, `https://github.com/${REPO_OWNER}/${REPO_NAME}`)}
           role="button"
           tabIndex={0}
        >
            <md-icon slot="icon"><svg viewBox="0 0 24 24"><path d={ICONS.github} /></svg></md-icon>
            {store.L.info.projectLink}
        </md-filled-tonal-button>

        <md-filled-tonal-button 
           class="action-btn donate-btn"
           onClick={(e: MouseEvent) => handleLink(e, DONATE_LINK)}
           role="button"
           tabIndex={0}
        >
            <md-icon slot="icon"><svg viewBox="0 0 24 24"><path d={ICONS.donate} /></svg></md-icon>
            {store.L.info.donate}
        </md-filled-tonal-button>

        <md-filled-tonal-button 
           class="action-btn"
           onClick={(e: MouseEvent) => handleLink(e, TELEGRAM_LINK)}
           role="button"
           tabIndex={0}
        >
            <md-icon slot="icon"><svg viewBox="0 0 24 24"><path d={ICONS.telegram} /></svg></md-icon>
            Telegram
        </md-filled-tonal-button>
      </div>

      <div class="contributors-section">
        <div class="section-title">{store.L.info.contributors}</div>
        
        <div class="list-wrapper">
          <Show when={!loading()} fallback={
            <For each={Array(3)}>{() =>
                  <div class="skeleton-item">
                      <Skeleton width="40px" height="40px" borderRadius="50%" />
                      <div class="skeleton-text">
                          <Skeleton width="120px" height="16px" />
                          <Skeleton width="180px" height="12px" />
                      </div>
                  </div>
            }</For>
          }>
            <Show when={!error()} fallback={
              <div class="error-message">
                  {store.L.info.loadFail}
              </div>
            }>
              <md-list class="contributors-list">
                <For each={contributors()}>
                  {(user) => (
                    <md-list-item 
                      type="link" 
                      href={user.html_url}
                      target="_blank"
                      onClick={(e: MouseEvent) => handleLink(e, user.html_url)}
                      role="link"
                      tabIndex={0}
                    >
                      <img slot="start" src={user.avatar_url} alt={user.login} class="c-avatar" loading="lazy" />
                      <div slot="headline">{user.name || user.login}</div>
                      <div slot="supporting-text">{user.bio || store.L.info.noBio}</div>
                    </md-list-item>
                  )}
                </For>
              </md-list>
            </Show>
          </Show>
        </div>
      </div>
    </div>
  );
}