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
  createEffect,
  createRenderEffect,
  createSignal,
  createMemo,
  onMount,
  onCleanup,
  Show,
  lazy,
  For,
} from "solid-js";
import { uiStore } from "./lib/stores/uiStore";
import { configStore } from "./lib/stores/configStore";
import { sysStore } from "./lib/stores/sysStore";
import { API } from "./lib/api";
import { getErrorMessage } from "./lib/api/core/error";
import { onSseStateUpdate, stopSse } from "./lib/api/core/bridge";
import { runStartupGate } from "./lib/appStartup";
import TopBar from "./components/TopBar";
import NavBar from "./components/NavBar";
import Toast from "./components/Toast";
import CleanReinstallRequired from "./components/CleanReinstallRequired";

type StartupState =
  | "checking-install"
  | "initializing"
  | "ready"
  | "clean-reinstall-required";

const loadStatusTab = () => import("./routes/StatusTab");
const loadConfigTab = () => import("./routes/ConfigTab");
const loadModulesTab = () => import("./routes/ModulesTab");
const loadInfoTab = () => import("./routes/InfoTab");

const routes = [
  { id: "status", load: loadStatusTab, component: lazy(loadStatusTab) },
  { id: "config", load: loadConfigTab, component: lazy(loadConfigTab) },
  { id: "modules", load: loadModulesTab, component: lazy(loadModulesTab) },
  { id: "info", load: loadInfoTab, component: lazy(loadInfoTab) },
];

export default function App() {
  const [activeTab, setActiveTab] = createSignal("status");
  const [dragOffset, setDragOffset] = createSignal(0);
  const [isDragging, setIsDragging] = createSignal(false);
  const [startupState, setStartupState] =
    createSignal<StartupState>("checking-install");
  const [initializationError, setInitializationError] = createSignal<
    string | null
  >(null);
  const [visitedTabs, setVisitedTabs] = createSignal(
    new Set<string>([activeTab()]),
  );

  let containerRef: HTMLDivElement | undefined;
  let containerWidth = 0;
  let touchStartX = 0;
  let touchStartY = 0;
  let ticking = false;
  let rafId: number | null = null;
  let cancelRoutePreload: (() => void) | undefined;
  const preloadedRouteIds = new Set<string>();
  let disposed = false;

  const visibleRoutes = createMemo(() => routes);
  const visibleTabs = createMemo(() => visibleRoutes().map((r) => r.id));
  const tabCount = createMemo(() => Math.max(visibleTabs().length, 1));
  const isAppReady = createMemo(() => startupState() === "ready");

  const baseTranslateX = createMemo(() => {
    const index = visibleTabs().indexOf(activeTab());
    return index >= 0 ? index * -(100 / tabCount()) : 0;
  });

  createRenderEffect(() => {
    const count = tabCount();
    const translate = baseTranslateX();
    const offset = dragOffset();
    const container = containerRef;
    if (!container) return;

    container.style.setProperty("--swipe-tab-count", String(count));
    container.style.setProperty("--swipe-base-translate", `${translate}%`);
    container.style.setProperty("--swipe-drag-offset", `${offset}px`);
  });

  createEffect(() => {
    const currentTab = activeTab();
    setVisitedTabs((prev) => {
      if (prev.has(currentTab)) return prev;
      const next = new Set(prev);
      next.add(currentTab);
      return next;
    });
  });

  createEffect(() => {
    const tabs = visibleTabs();
    if (!tabs.includes(activeTab())) {
      setActiveTab(tabs.includes("config") ? "config" : tabs[0] || "status");
    }
  });

  function handleTouchStart(e: TouchEvent) {
    touchStartX = e.changedTouches[0].screenX;
    touchStartY = e.changedTouches[0].screenY;
    setIsDragging(true);
    setDragOffset(0);
    ticking = false;
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
  }

  function handleTouchMove(e: TouchEvent) {
    if (!isDragging()) return;
    const currentX = e.changedTouches[0].screenX;
    const currentY = e.changedTouches[0].screenY;
    let diffX = currentX - touchStartX;
    const diffY = currentY - touchStartY;

    if (Math.abs(diffY) > Math.abs(diffX)) return;
    if (e.cancelable) e.preventDefault();

    if (!ticking) {
      ticking = true;
      rafId = requestAnimationFrame(() => {
        ticking = false;
        rafId = null;
        if (!isDragging()) return;
        const tabs = visibleTabs();
        const currentIndex = tabs.indexOf(activeTab());
        if (
          (currentIndex === 0 && diffX > 0) ||
          (currentIndex === tabs.length - 1 && diffX < 0)
        ) {
          diffX = diffX / 3;
        }
        setDragOffset(diffX);
      });
    }
  }

  function handleTouchEnd() {
    if (!isDragging()) return;
    setIsDragging(false);
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
      ticking = false;
    }
    if (containerRef) containerWidth = containerRef.clientWidth;
    const threshold = containerWidth * 0.33 || 80;
    const tabs = visibleTabs();
    const currentIndex = tabs.indexOf(activeTab());
    let nextIndex = currentIndex;
    const currentOffset = dragOffset();

    if (currentOffset < -threshold && currentIndex < tabs.length - 1) {
      nextIndex = currentIndex + 1;
    } else if (currentOffset > threshold && currentIndex > 0) {
      nextIndex = currentIndex - 1;
    }
    if (nextIndex !== currentIndex) changeActiveTab(tabs[nextIndex]);
    setDragOffset(0);
  }

  onCleanup(() => {
    disposed = true;
    stopSse();
    cancelRoutePreload?.();
  });

  function scheduleIdleTask(callback: () => void, timeout = 1500) {
    if ("requestIdleCallback" in window) {
      const idleId = window.requestIdleCallback(callback, { timeout });
      return () => window.cancelIdleCallback(idleId);
    }

    const timerId = globalThis.setTimeout(callback, Math.min(timeout, 300));
    return () => globalThis.clearTimeout(timerId);
  }

  function scheduleAdjacentRoutePreload(tabId = activeTab(), timeout = 2500) {
    cancelRoutePreload?.();

    const tabs = visibleTabs();
    const currentIndex = tabs.indexOf(tabId);
    if (currentIndex < 0) return;

    const routeById = new Map(
      visibleRoutes().map((route) => [route.id, route]),
    );
    const pendingRoutes = [tabs[currentIndex + 1], tabs[currentIndex - 1]]
      .filter((id): id is string => Boolean(id))
      .filter((id) => !preloadedRouteIds.has(id))
      .map((id) => routeById.get(id))
      .filter((route): route is (typeof routes)[number] => Boolean(route));
    let nextIndex = 0;

    const preloadNextRoute = () => {
      if (disposed) return;

      const nextRoute = pendingRoutes[nextIndex++];
      if (!nextRoute) return;

      preloadedRouteIds.add(nextRoute.id);
      void nextRoute.load();

      if (nextIndex < pendingRoutes.length) {
        cancelRoutePreload = scheduleIdleTask(preloadNextRoute);
      }
    };

    cancelRoutePreload = scheduleIdleTask(preloadNextRoute, timeout);
  }

  function changeActiveTab(tabId: string) {
    if (tabId === activeTab()) return;
    setActiveTab(tabId);
    scheduleAdjacentRoutePreload(tabId);
  }

  onMount(() => {
    void initializeApp();
  });

  async function initializeApp() {
    try {
      const startup = await runStartupGate(API, () => {
        if (disposed) return false;
        setStartupState("initializing");
        return true;
      });
      if (disposed || startup.state === "cancelled") return;
      if (startup.state === "clean-reinstall-required") {
        setStartupState("clean-reinstall-required");
        return;
      }

      const payload = startup.payload;
      sysStore.loadFromInit(payload);
      configStore.loadFromInit(payload);
      if (disposed) return;
      onSseStateUpdate((event) => sysStore.handleSseUpdate(event.payload));
      setStartupState("ready");
      scheduleAdjacentRoutePreload(activeTab(), 4000);
    } catch (e: unknown) {
      console.error("App initialization failed", e);
      const message = getErrorMessage(e, "App initialization failed");
      setInitializationError(message);
      uiStore.showToast(message, "error");
    }
  }

  return (
    <div class="app-root">
      <TopBar />
      <Show
        when={startupState() !== "clean-reinstall-required"}
        fallback={<CleanReinstallRequired />}
      >
        <main
          class="main-content"
          ref={containerRef}
          onTouchStart={handleTouchStart}
          onTouchMove={handleTouchMove}
          onTouchEnd={handleTouchEnd}
          onTouchCancel={handleTouchEnd}
        >
          <Show
            when={!initializationError()}
            fallback={
              <div class="loading-container" style={{ height: "100%" }}>
                <span class="loading-text">{initializationError()}</span>
              </div>
            }
          >
            <Show
              when={isAppReady()}
              fallback={
                <div class="loading-container" style={{ height: "100%" }}>
                  <div class="spinner"></div>
                  <span class="loading-text">Loading...</span>
                </div>
              }
            >
              <div
                class="swipe-track"
                classList={{ "is-dragging": isDragging() }}
              >
                <For each={visibleRoutes()}>
                  {(route) => (
                    <div class="swipe-page">
                      <Show
                        when={visitedTabs().has(route.id)}
                        fallback={
                          <div class="page-scroller" aria-hidden="true" />
                        }
                      >
                        <div class="page-scroller">
                          <route.component />
                        </div>
                      </Show>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </Show>
        </main>
        <Show when={startupState() === "ready"}>
          <NavBar
            activeTab={activeTab()}
            onTabChange={changeActiveTab}
            tabs={visibleRoutes()}
          />
        </Show>
      </Show>
      <Toast />
    </div>
  );
}
