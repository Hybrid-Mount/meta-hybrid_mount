/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import {
  createSignal,
  createMemo,
  createEffect,
  onMount,
  onCleanup,
  Show,
  For,
  createDeferred,
} from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import { moduleStore } from "../lib/stores/moduleStore";
import { sysStore } from "../lib/stores/sysStore";
import { ICONS } from "../lib/constants";
import { ENABLE_KASUMI } from "../lib/constants_gen";
import { features } from "../lib/features";
import Skeleton from "../components/Skeleton";
import BottomActions from "../components/BottomActions";
import type { Module, MountMode } from "../lib/types";
import "./ModulesTab.css";
import "@material/web/iconbutton/filled-tonal-icon-button.js";
import "@material/web/button/filled-tonal-button.js";
import "@material/web/icon/icon.js";

export default function ModulesTab() {
  const BATCH_SIZE = 20;
  const [searchQuery, setSearchQuery] = createSignal("");
  const deferredSearchQuery = createDeferred(searchQuery);
  const [filterType, setFilterType] = createSignal<
    "all" | MountMode | "blacklisted"
  >("all");
  const [showUnmounted, setShowUnmounted] = createSignal(false);
  const [expandedId, setExpandedId] = createSignal<string | null>(null);
  const [visibleCount, setVisibleCount] = createSignal(BATCH_SIZE);
  let observerTarget: HTMLDivElement | undefined;

  onMount(() => {
    load();
    const observerRoot = observerTarget?.closest(".page-scroller") ?? undefined;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          setVisibleCount((count) => count + BATCH_SIZE);
        }
      },
      { root: observerRoot, rootMargin: "200px" },
    );
    if (observerTarget) observer.observe(observerTarget);
    onCleanup(() => observer.disconnect());
  });

  createEffect(() => {
    searchQuery();
    filterType();
    showUnmounted();
    setVisibleCount(BATCH_SIZE);
  });

  const kasumiMasterEnabled = createMemo(
    () => ENABLE_KASUMI && features.kasumiEnabled,
  );
  const kasumiAvailable = createMemo(
    () => ENABLE_KASUMI && features.kasumiAvailable,
  );
  const tmpfsXattrUnsupported = createMemo(
    () => !sysStore.systemInfo.tmpfs_xattr_supported,
  );
  const showKasumiStrategy = createMemo(
    () => kasumiMasterEnabled() && !tmpfsXattrUnsupported(),
  );

  createEffect(() => {
    if (!showKasumiStrategy() && filterType() === "kasumi") {
      setFilterType("all");
    }
  });

  function load(force = false) {
    void moduleStore.loadModules(force);
  }

  function updateModule(modId: string, transform: (m: Module) => Module) {
    const idx = moduleStore.modules.findIndex((m) => m.id === modId);
    if (idx === -1) return;

    const newModules = [...moduleStore.modules];
    newModules[idx] = transform({ ...newModules[idx] });
    moduleStore.modules = newModules;
  }

  async function updateDefaultMode(mod: Module, mode: MountMode) {
    if (mod.rules.default_mode === mode) return;

    const newRules = { ...mod.rules, default_mode: mode };
    updateModuleRules(mod.id, () => newRules);
    const saved = await moduleStore.saveModuleRules(mod.id, newRules);
    if (!saved) {
      await moduleStore.loadModules(true);
    }
  }

  const filteredModules = createMemo(() => {
    const q = deferredSearchQuery().trim().toLowerCase();
    const currentFilter = filterType();
    const includeUnmounted = showUnmounted();

    return moduleStore.modules.filter((module) => {
      if (!module.is_mounted && !includeUnmounted && !module.is_blacklisted) {
        return false;
      }
      if (
        q &&
        !module.name.toLowerCase().includes(q) &&
        !module.id.toLowerCase().includes(q)
      ) {
        return false;
      }
      if (currentFilter === "blacklisted") {
        if (!module.is_blacklisted) return false;
      } else if (currentFilter !== "all" && module.mode !== currentFilter) {
        return false;
      }

      return true;
    });
  });

  const canLoadMore = createMemo(
    () => visibleCount() < filteredModules().length,
  );

  function loadMore() {
    setVisibleCount((count) => count + BATCH_SIZE);
  }

  function toggleExpand(id: string) {
    if (expandedId() === id) {
      setExpandedId(null);
    } else {
      setExpandedId(id);
    }
  }

  const MODE_DISPLAY: Record<string, { label: () => string; cls: string }> = {
    blacklisted: {
      label: () => uiStore.L.modules.modes.blacklisted,
      cls: "mode-blacklisted",
    },
    unmounted: {
      label: () => uiStore.L.modules.modes.unmounted,
      cls: "mode-ignore",
    },
    magic: {
      label: () => uiStore.L.modules.modes.magic,
      cls: "mode-magic",
    },
    kasumi: {
      label: () => uiStore.L.modules.modes.kasumi,
      cls: "mode-kasumi",
    },
    overlay: {
      label: () => uiStore.L.modules.modes.overlay,
      cls: "mode-overlay",
    },
  };

  function getModeInfo(mod: Module): { label: string; cls: string } {
    const key = mod.is_blacklisted
      ? "blacklisted"
      : !mod.is_mounted
        ? "unmounted"
        : mod.mode;
    const entry = MODE_DISPLAY[key];
    return { label: entry.label(), cls: entry.cls };
  }

  function getEffectiveDefaultMode(mod: Module): MountMode {
    return mod.rules.default_mode;
  }

  function updateModuleRules(
    modId: string,
    updateFn: (rules: Module["rules"]) => Module["rules"],
  ) {
    updateModule(modId, (module) => ({
      ...module,
      rules: updateFn(module.rules),
    }));
  }

  return (
    <>
      <div class="modules-page">
        <div class="header-section">
          <div class="search-bar">
            <svg class="search-icon" viewBox="0 0 24 24">
              <path d={ICONS.search} />
            </svg>
            <input
              type="text"
              class="search-input"
              placeholder={uiStore.L.modules.searchPlaceholder}
              aria-label={uiStore.L.modules.searchPlaceholder}
              value={searchQuery()}
              onInput={(e) => setSearchQuery(e.currentTarget.value)}
            />

            <div class="filter-group">
              <button
                class={`btn-icon ${showUnmounted() ? "active" : ""}`}
                onClick={() => setShowUnmounted(!showUnmounted())}
                title={showUnmounted() ? "Hide Unmounted" : "Show Unmounted"}
                type="button"
                aria-pressed={showUnmounted()}
              >
                <svg viewBox="0 0 24 24" width="20" height="20">
                  <path
                    d={
                      showUnmounted() ? ICONS.visibility : ICONS.visibility_off
                    }
                    fill="currentColor"
                  />
                </svg>
              </button>

              <select
                class="filter-select"
                value={filterType()}
                onChange={(e) =>
                  setFilterType(
                    e.currentTarget.value as "all" | MountMode | "blacklisted",
                  )
                }
                aria-label={uiStore.L.modules.filterLabel}
                title={uiStore.L.modules.filterLabel}
              >
                <option value="all">{uiStore.L.modules.filterAll}</option>
                <option value="overlay">
                  {uiStore.L.modules.modes.short.overlay}
                </option>
                <option value="magic">
                  {uiStore.L.modules.modes.short.magic}
                </option>
                <Show when={ENABLE_KASUMI && showKasumiStrategy()}>
                  <option value="kasumi">
                    {uiStore.L.modules.modes.short.kasumi}
                  </option>
                </Show>
                <option value="blacklisted">
                  {uiStore.L.modules.modes.blacklisted}
                </option>
              </select>
            </div>
          </div>
        </div>

        <div class="modules-list">
          <Show
            when={!moduleStore.loading}
            fallback={
              <For each={Array(6)}>
                {() => <Skeleton variant="module-card" />}
              </For>
            }
          >
            <Show
              when={filteredModules().length > 0}
              fallback={
                <div class="empty-state">
                  <div class="empty-icon">
                    <md-icon>
                      <svg viewBox="0 0 24 24">
                        <path d={ICONS.modules} />
                      </svg>
                    </md-icon>
                  </div>
                  <div>{uiStore.L.modules.emptyState}</div>
                  <Show when={!showUnmounted()}>
                    <div class="empty-state-hint">
                      {uiStore.L.modules.unmountedHiddenHint}
                    </div>
                  </Show>
                </div>
              }
            >
              <For each={filteredModules().slice(0, visibleCount())}>
                {(mod) => {
                  const effectiveDefaultMode = () =>
                    getEffectiveDefaultMode(mod);
                  return (
                    <div
                      class={`module-card ${expandedId() === mod.id ? "expanded" : ""} ${mod.is_mounted ? "" : "unmounted"}`}
                    >
                      <button
                        class="module-header"
                        onClick={() => toggleExpand(mod.id)}
                        type="button"
                        aria-expanded={expandedId() === mod.id}
                      >
                        <div class="module-info">
                          <div class="module-name">{mod.name}</div>
                          <div class="module-meta">
                            <span class="module-id">{mod.id}</span>
                            <span class="version-badge">{mod.version}</span>
                          </div>
                        </div>
                        <div class="mode-group">
                          <div class={`mode-indicator ${getModeInfo(mod).cls}`}>
                            {getModeInfo(mod).label}
                          </div>
                          <Show when={mod.is_blacklisted}>
                            <div class="error-indicator blacklisted-indicator">
                              BLACKLIST
                            </div>
                          </Show>
                        </div>
                      </button>

                      <div class="module-body-wrapper">
                        <div class="module-body-inner">
                          <div class="module-body-content">
                            <p class="module-desc">{mod.description}</p>

                            <div class="body-section">
                              <div class="section-label">
                                {uiStore.L.modules.defaultMode}
                              </div>
                              <div class="strategy-selector">
                                <button
                                  class={`strategy-option ${effectiveDefaultMode() === "overlay" ? "selected" : ""}`}
                                  onClick={() =>
                                    updateDefaultMode(mod, "overlay")
                                  }
                                  type="button"
                                >
                                  <span class="opt-title">
                                    {uiStore.L.modules.modes.short.overlay}
                                  </span>
                                  <span class="opt-sub">
                                    {uiStore.L.modules.defaultTag}
                                  </span>
                                </button>
                                <button
                                  class={`strategy-option ${effectiveDefaultMode() === "magic" ? "selected" : ""}`}
                                  onClick={() =>
                                    updateDefaultMode(mod, "magic")
                                  }
                                  type="button"
                                >
                                  <span class="opt-title">
                                    {uiStore.L.modules.modes.short.magic}
                                  </span>
                                  <span class="opt-sub">
                                    {uiStore.L.modules.compatTag}
                                  </span>
                                </button>
                                <Show
                                  when={ENABLE_KASUMI && showKasumiStrategy()}
                                >
                                  <button
                                    class={`strategy-option ${effectiveDefaultMode() === "kasumi" ? "selected" : ""}`}
                                    onClick={() =>
                                      updateDefaultMode(mod, "kasumi")
                                    }
                                    disabled={!kasumiAvailable()}
                                    title={
                                      !kasumiAvailable()
                                        ? uiStore.L.modules
                                            .kasumiUnavailableHint
                                        : undefined
                                    }
                                    type="button"
                                  >
                                    <span class="opt-title">
                                      {uiStore.L.modules.modes.short.kasumi}
                                    </span>
                                    <span class="opt-sub">
                                      {!kasumiAvailable()
                                        ? uiStore.L.modules.unavailableTag
                                        : uiStore.L.modules.nativeTag}
                                    </span>
                                  </button>
                                </Show>
                                <button
                                  class={`strategy-option ${effectiveDefaultMode() === "ignore" ? "selected" : ""}`}
                                  onClick={() =>
                                    updateDefaultMode(mod, "ignore")
                                  }
                                  type="button"
                                >
                                  <span class="opt-title">
                                    {uiStore.L.modules.modes.short.ignore}
                                  </span>
                                  <span class="opt-sub">
                                    {uiStore.L.modules.disableTag}
                                  </span>
                                </button>
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  );
                }}
              </For>
              <div ref={observerTarget} class="observer-sentinel"></div>
            </Show>
          </Show>
        </div>
      </div>

      <BottomActions>
        <Show when={canLoadMore()}>
          <md-filled-tonal-button onClick={loadMore}>
            {uiStore.L.modules.loadMore}
          </md-filled-tonal-button>
        </Show>

        <md-filled-tonal-icon-button
          onClick={() => load(true)}
          disabled={moduleStore.loading}
          title={uiStore.L.modules.reload}
        >
          <md-icon>
            <svg viewBox="0 0 24 24">
              <path d={ICONS.refresh} />
            </svg>
          </md-icon>
        </md-filled-tonal-icon-button>
      </BottomActions>
    </>
  );
}
