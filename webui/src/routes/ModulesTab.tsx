/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
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
import { ICONS } from "../lib/constants";
import Skeleton from "../components/Skeleton";
import BottomActions from "../components/BottomActions";
import { API } from "../lib/api";
import type { Module, MountMode } from "../lib/types";
import "./ModulesTab.css";
import "@material/web/iconbutton/filled-tonal-icon-button.js";
import "@material/web/button/filled-button.js";
import "@material/web/icon/icon.js";

export default function ModulesTab() {
  const [searchQuery, setSearchQuery] = createSignal("");
  const deferredSearchQuery = createDeferred(searchQuery);
  const [filterType, setFilterType] = createSignal("all");
  const [showUnmounted, setShowUnmounted] = createSignal(false);
  const [expandedId, setExpandedId] = createSignal<string | null>(null);
  const [initialRulesSnapshot, setInitialRulesSnapshot] = createSignal<
    Record<string, string>
  >({});
  const [isSaving, setIsSaving] = createSignal(false);
  const [visibleCount, setVisibleCount] = createSignal(20);
  let observerTarget: HTMLDivElement | undefined;

  onMount(() => {
    load();
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          setVisibleCount((c) => c + 20);
        }
      },
      { rootMargin: "200px" },
    );
    if (observerTarget) observer.observe(observerTarget);
    onCleanup(() => observer.disconnect());
  });

  createEffect(() => {
    searchQuery();
    filterType();
    showUnmounted();
    setVisibleCount(20);
  });

  function load() {
    moduleStore.loadModules().then(() => {
      const snapshot: Record<string, string> = {};
      moduleStore.modules.forEach((m) => {
        snapshot[m.id] = JSON.stringify(m.rules);
      });
      setInitialRulesSnapshot(snapshot);
    });
  }

  const dirtyModules = createMemo(() =>
    moduleStore.modules.filter((m) => {
      const initial = initialRulesSnapshot()[m.id];
      if (!initial) return false;
      return JSON.stringify(m.rules) !== initial;
    }),
  );

  const isDirty = createMemo(() => dirtyModules().length > 0);

  function updateModule(modId: string, transform: (m: Module) => Module) {
    const idx = moduleStore.modules.findIndex((m) => m.id === modId);
    if (idx === -1) return;

    const newModules = [...moduleStore.modules];
    newModules[idx] = transform({ ...newModules[idx] });
    moduleStore.modules = newModules;
  }

  async function performSave() {
    setIsSaving(true);
    try {
      const dirty = dirtyModules();
      for (const mod of dirty) {
        await API.saveModuleRules(mod.id, mod.rules);
      }
      await load();
      uiStore.showToast(
        uiStore.L.modules?.saveSuccess || "Saved successfully",
        "success",
      );
    } catch (e: any) {
      uiStore.showToast(e.message || "Failed to save", "error");
    } finally {
      setIsSaving(false);
    }
  }

  const filteredModules = createMemo(() =>
    moduleStore.modules.filter((m) => {
      const q = deferredSearchQuery().toLowerCase();
      if (!m.is_mounted && !showUnmounted()) {
        return false;
      }
      const matchSearch =
        m.name.toLowerCase().includes(q) || m.id.toLowerCase().includes(q);
      if (!matchSearch) return false;
      if (filterType() !== "all" && m.mode !== filterType()) {
        return false;
      }

      return true;
    }),
  );

  function toggleExpand(id: string) {
    if (expandedId() === id) {
      setExpandedId(null);
    } else {
      setExpandedId(id);
    }
  }

  function getModeLabel(mod: Module) {
    const m = uiStore.L.modules?.modes;
    if (!mod.is_mounted) return m?.none ?? "Unmounted";
    if (mod.mode === "magic") return m?.magic ?? "Magic";
    return m?.auto ?? "Overlay";
  }

  function getModeClass(mod: Module) {
    if (!mod.is_mounted) return "mode-ignore";
    if (mod.mode === "magic") return "mode-magic";
    return "mode-auto";
  }

  function updateModuleRules(
    modId: string,
    updateFn: (rules: Module["rules"]) => Module["rules"],
  ) {
    updateModule(modId, (m) => ({ ...m, rules: updateFn(m.rules) }));
  }

  function updateDefaultMode(mod: Module, mode: MountMode) {
    updateModuleRules(mod.id, (rules) => ({ ...rules, default_mode: mode }));
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
              placeholder={uiStore.L.modules?.searchPlaceholder}
              aria-label={
                uiStore.L.modules?.searchPlaceholder || "Search modules"
              }
              value={searchQuery()}
              onInput={(e) => setSearchQuery(e.currentTarget.value)}
            />

            <div class="filter-group">
              <button
                class="btn-icon"
                onClick={() => setShowUnmounted(!showUnmounted())}
                title={showUnmounted() ? "Hide unmounted" : "Show unmounted"}
                style={{
                  color: showUnmounted()
                    ? "var(--md-sys-color-primary)"
                    : "var(--md-sys-color-outline)",
                }}
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
                onChange={(e) => setFilterType(e.currentTarget.value)}
                aria-label={uiStore.L.modules?.filterLabel || "Filter modules"}
                title={uiStore.L.modules?.filterLabel || "Filter modules"}
              >
                <option value="all">{uiStore.L.modules?.filterAll}</option>
                <option value="auto">Overlay</option>
                <option value="magic">Magic</option>
              </select>
            </div>
          </div>
        </div>

        <div class="modules-list">
          <Show
            when={!moduleStore.loading}
            fallback={
              <For each={Array(6)}>
                {() => <Skeleton height="64px" borderRadius="16px" />}
              </For>
            }
          >
            <Show
              when={filteredModules().length > 0}
              fallback={
                <div class="empty-state">
                  <div>No modules found.</div>
                  <Show when={!showUnmounted()}>
                    <div
                      style={{
                        "font-size": "12px",
                        opacity: "0.7",
                        "margin-top": "8px",
                      }}
                    >
                      Unmounted modules are hidden.
                    </div>
                  </Show>
                </div>
              }
            >
              <For each={filteredModules().slice(0, visibleCount())}>
                {(mod) => (
                  <div
                    class={`module-card ${expandedId() === mod.id ? "expanded" : ""} ${initialRulesSnapshot()[mod.id] !== JSON.stringify(mod.rules) ? "dirty" : ""}`}
                  >
                    <div
                      class="module-header"
                      onClick={() => toggleExpand(mod.id)}
                      role="button"
                      tabIndex={0}
                      onKeyDown={(e) =>
                        e.key === "Enter" && toggleExpand(mod.id)
                      }
                    >
                      <div class="module-info">
                        <div class="module-name">{mod.name}</div>
                        <div class="module-meta">
                          <span class="module-id">{mod.id}</span>
                          <span class="version-badge">{mod.version}</span>
                        </div>
                      </div>
                      <div class={`mode-indicator ${getModeClass(mod)}`}>
                        {getModeLabel(mod)}
                      </div>
                    </div>

                    <div class="module-body-wrapper">
                      <div class="module-body-inner">
                        <div class="module-body-content">
                          <p class="module-desc">{mod.description}</p>

                          <div class="body-section">
                            <div class="section-label">
                              {uiStore.L.modules?.defaultMode ?? "Strategy"}
                            </div>
                            <div class="strategy-selector">
                              <button
                                class={`strategy-option ${mod.rules.default_mode === "overlay" ? "selected" : ""}`}
                                onClick={() =>
                                  updateDefaultMode(mod, "overlay")
                                }
                              >
                                <span class="opt-title">
                                  {uiStore.L.modules?.modes?.short?.auto ??
                                    "Overlay"}
                                </span>
                                <span class="opt-sub">Default</span>
                              </button>
                              <button
                                class={`strategy-option ${mod.rules.default_mode === "magic" ? "selected" : ""}`}
                                onClick={() => updateDefaultMode(mod, "magic")}
                              >
                                <span class="opt-title">
                                  {uiStore.L.modules?.modes?.short?.magic ??
                                    "Magic"}
                                </span>
                                <span class="opt-sub">Compat</span>
                              </button>
                              <button
                                class={`strategy-option ${mod.rules.default_mode === "ignore" ? "selected" : ""}`}
                                onClick={() => updateDefaultMode(mod, "ignore")}
                              >
                                <span class="opt-title">
                                  {uiStore.L.modules?.modes?.short?.ignore ??
                                    "Ignore"}
                                </span>
                                <span class="opt-sub">Disable</span>
                              </button>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                )}
              </For>
              <div ref={observerTarget} style={{ height: "20px" }}></div>
            </Show>
          </Show>
        </div>
      </div>

      <BottomActions>
        <md-filled-tonal-icon-button
          onClick={load}
          disabled={moduleStore.loading}
          title={uiStore.L.modules?.reload}
        >
          <md-icon>
            <svg viewBox="0 0 24 24">
              <path d={ICONS.refresh} />
            </svg>
          </md-icon>
        </md-filled-tonal-icon-button>

        <div class="spacer"></div>

        <md-filled-button
          onClick={performSave}
          disabled={isSaving() || !isDirty()}
        >
          <md-icon slot="icon">
            <svg viewBox="0 0 24 24">
              <path d={ICONS.save} />
            </svg>
          </md-icon>
          {isSaving() ? uiStore.L.common?.saving : uiStore.L.modules?.save}
        </md-filled-button>
      </BottomActions>
    </>
  );
}
