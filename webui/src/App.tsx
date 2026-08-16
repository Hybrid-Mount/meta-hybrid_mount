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
import TopBar from "./components/TopBar";
import NavBar from "./components/NavBar";
import Toast from "./components/Toast";

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
  const [initialDataReady, setInitialDataReady] = createSignal(false);
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
  let disposed = false;

  const visibleRoutes = createMemo(() => routes);
  const visibleTabs = createMemo(() => visibleRoutes().map((r) => r.id));
  const tabCount = createMemo(() => Math.max(visibleTabs().length, 1));
  const isAppReady = createMemo(() => initialDataReady());

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
    if (nextIndex !== currentIndex) setActiveTab(tabs[nextIndex]);
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

  function startRoutePreload() {
    const pendingRoutes = visibleRoutes().filter(
      (route) => route.id !== activeTab(),
    );
    let nextIndex = 0;

    const preloadNextRoute = () => {
      if (disposed) return;

      const nextRoute = pendingRoutes[nextIndex++];
      if (!nextRoute) return;

      void nextRoute.load();

      if (nextIndex < pendingRoutes.length) {
        cancelRoutePreload = scheduleIdleTask(preloadNextRoute);
      }
    };

    cancelRoutePreload = scheduleIdleTask(preloadNextRoute);
  }

  onMount(() => {
    void initializeApp();
  });

  async function initializeApp() {
    try {
      const [payload] = await Promise.all([API.init(), uiStore.init()]);
      if (disposed) return;
      sysStore.loadFromInit(payload);
      onSseStateUpdate((state) => sysStore.handleSseUpdate(state));
      configStore.loadFromInit(payload);
      setInitialDataReady(true);
      startRoutePreload();
    } catch (e: unknown) {
      console.error("App initialization failed", e);
      uiStore.showToast(
        getErrorMessage(e, "App initialization failed"),
        "error",
      );
      setInitialDataReady(true);
      return;
    }
  }

  return (
    <div class="app-root">
      <TopBar />
      <main
        class="main-content"
        ref={containerRef}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
        onTouchCancel={handleTouchEnd}
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
          <div class="swipe-track" classList={{ "is-dragging": isDragging() }}>
            <For each={visibleRoutes()}>
              {(route) => (
                <div class="swipe-page">
                  <Show
                    when={visitedTabs().has(route.id)}
                    fallback={<div class="page-scroller" aria-hidden="true" />}
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
      </main>
      <NavBar
        activeTab={activeTab()}
        onTabChange={setActiveTab}
        tabs={visibleRoutes()}
      />
      <Toast />
    </div>
  );
}
