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
import { API } from "../lib/api";
import Skeleton from "../components/Skeleton";
import BottomActions from "../components/BottomActions";
import type { Module, MountMode } from "../lib/types";
import "./ModulesTab.css";
import { getErrorMessage } from "../lib/api/core/error";
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
    () => sysStore.systemInfo?.tmpfs_xattr_supported === false,
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
    const newRules = { ...mod.rules, default_mode: mode };
    updateModuleRules(mod.id, () => newRules);
    const saved = await moduleStore.saveModules();
    if (!saved) {
      await moduleStore.loadModules(true);
    }
  }

  const filteredModules = createMemo(() => {
    const q = deferredSearchQuery().trim().toLowerCase();
    const currentFilter = filterType();
    const includeUnmounted = showUnmounted();

    return moduleStore.modules.filter((module) => {
      const hasMountError = Boolean(module.mount_error);
      if (!module.is_mounted && !includeUnmounted && !hasMountError) {
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
        if (module.mount_error !== "blacklisted") return false;
      } else if (currentFilter !== "all" && module.mode !== currentFilter) {
        return false;
      }

      return true;
    });
  });
  const [clearingErrors, setClearingErrors] = createSignal(false);
  const hasMountErrors = createMemo(() =>
    moduleStore.modules.some((m) => !!m.mount_error),
  );

  async function clearMountErrors() {
    setClearingErrors(true);
    try {
      await API.clearMountErrors();
      uiStore.showToast(
        uiStore.L.modules?.mountErrorsCleared || "Mount errors cleared",
        "success",
      );
      await moduleStore.loadModules(true);
    } catch (e: unknown) {
      uiStore.showToast(
        getErrorMessage(
          e,
          uiStore.L.modules?.mountErrorsClearFailed ??
            "Failed to clear mount errors",
        ),
        "error",
      );
    } finally {
      setClearingErrors(false);
    }
  }

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
      label: () => uiStore.L.modules?.modes?.blacklisted ?? "Blacklisted",
      cls: "mode-blacklisted",
    },
    unmounted: {
      label: () => uiStore.L.modules?.modes?.unmounted ?? "Unmounted",
      cls: "mode-ignore",
    },
    magic: {
      label: () => uiStore.L.modules?.modes?.magic ?? "Magic",
      cls: "mode-magic",
    },
    kasumi: {
      label: () => uiStore.L.modules?.modes?.kasumi ?? "Kasumi",
      cls: "mode-kasumi",
    },
    overlay: {
      label: () => uiStore.L.modules?.modes?.overlay ?? "OverlayFS",
      cls: "mode-overlay",
    },
  };

  function getModeInfo(mod: Module): { label: string; cls: string } {
    const key =
      mod.mount_error === "blacklisted"
        ? "blacklisted"
        : !mod.is_mounted
          ? "unmounted"
          : mod.mode;
    const entry = MODE_DISPLAY[key] ?? MODE_DISPLAY.overlay;
    return { label: entry.label(), cls: entry.cls };
  }

  function getEffectiveDefaultMode(mod: Module): MountMode {
    const mode = mod.rules.default_mode;
    if (mode === "kasumi" && !kasumiAvailable()) {
      return "ignore";
    }
    return mode;
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
              placeholder={uiStore.L.modules?.searchPlaceholder}
              aria-label={
                uiStore.L.modules?.searchPlaceholder || "Search modules"
              }
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
                aria-label={uiStore.L.modules?.filterLabel || "Filter modules"}
                title={uiStore.L.modules?.filterLabel || "Filter modules"}
              >
                <option value="all">{uiStore.L.modules?.filterAll}</option>
                <option value="overlay">
                  {uiStore.L.modules?.modes?.short?.overlay ?? "Overlay"}
                </option>
                <option value="magic">
                  {uiStore.L.modules?.modes?.short?.magic ?? "Magic"}
                </option>
                <Show when={ENABLE_KASUMI && showKasumiStrategy()}>
                  <option value="kasumi">
                    {uiStore.L.modules?.modes?.short?.kasumi ?? "Kasumi"}
                  </option>
                </Show>
                <option value="blacklisted">
                  {uiStore.L.modules?.modes?.blacklisted ?? "Blacklisted"}
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
                  <div>
                    {uiStore.L.modules?.emptyState ?? "No modules found."}
                  </div>
                  <Show when={!showUnmounted()}>
                    <div class="empty-state-hint">
                      {uiStore.L.modules?.unmountedHiddenHint ??
                        "Unmounted modules are hidden."}
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
                          <Show when={mod.mount_error}>
                            <div
                              class={`error-indicator ${mod.mount_error === "blacklisted" ? "blacklisted-indicator" : ""}`}
                              title={mod.mount_error}
                            >
                              {mod.mount_error === "blacklisted"
                                ? "BLACKLIST"
                                : "ERROR"}
                            </div>
                          </Show>
                        </div>
                      </button>

                      <div class="module-body-wrapper">
                        <div class="module-body-inner">
                          <div class="module-body-content">
                            <p class="module-desc">{mod.description}</p>

                            <Show when={mod.mount_error}>
                              <div class="error-banner">
                                <svg
                                  class="error-icon"
                                  viewBox="0 0 24 24"
                                  width="16"
                                  height="16"
                                >
                                  <path d={ICONS.bug} fill="currentColor" />
                                </svg>
                                <div class="error-content">
                                  <span class="error-text">
                                    {uiStore.L.modules?.mountError ||
                                      "Mount Error"}
                                    : {mod.mount_error}
                                  </span>
                                  <Show when={mod.suggest_ignore}>
                                    <span class="suggest-ignore-hint">
                                      {uiStore.L.modules?.suggestIgnoreHint ??
                                        "This module contains mount-related commands in its .sh files. Consider setting its mode to 'Ignore'."}
                                    </span>
                                  </Show>
                                </div>
                              </div>
                            </Show>

                            <div class="body-section">
                              <div class="section-label">
                                {uiStore.L.modules?.defaultMode ?? "Strategy"}
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
                                    {uiStore.L.modules?.modes?.short?.overlay ??
                                      "Overlay"}
                                  </span>
                                  <span class="opt-sub">
                                    {uiStore.L.modules?.defaultTag ?? "Default"}
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
                                    {uiStore.L.modules?.modes?.short?.magic ??
                                      "Magic"}
                                  </span>
                                  <span class="opt-sub">
                                    {uiStore.L.modules?.compatTag ?? "Compat"}
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
                                        ? (uiStore.L.modules
                                            ?.kasumiUnavailableHint ??
                                          "Kasumi is not currently available")
                                        : undefined
                                    }
                                    type="button"
                                  >
                                    <span class="opt-title">
                                      {uiStore.L.modules?.modes?.short
                                        ?.kasumi ?? "Kasumi"}
                                    </span>
                                    <span class="opt-sub">
                                      {!kasumiAvailable()
                                        ? (uiStore.L.modules?.unavailableTag ??
                                          "Unavailable")
                                        : (uiStore.L.modules?.nativeTag ??
                                          "Stealth")}
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
                                    {uiStore.L.modules?.modes?.short?.ignore ??
                                      "Ignore"}
                                  </span>
                                  <span class="opt-sub">
                                    {uiStore.L.modules?.disableTag ?? "Disable"}
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
        <Show when={hasMountErrors()}>
          <md-filled-tonal-button
            onClick={clearMountErrors}
            disabled={clearingErrors()}
          >
            {uiStore.L.modules?.clearMountErrors ?? "Clear Mount Errors"}
          </md-filled-tonal-button>
        </Show>

        <Show when={canLoadMore()}>
          <md-filled-tonal-button onClick={loadMore}>
            {uiStore.L.modules?.loadMore ?? "Load More"}
          </md-filled-tonal-button>
        </Show>

        <md-filled-tonal-icon-button
          onClick={() => load(true)}
          disabled={moduleStore.loading}
          title={uiStore.L.modules?.reload}
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
