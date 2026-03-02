/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createMemo, onMount, Show, lazy, For } from "solid-js";
import { uiStore } from "./lib/stores/uiStore";
import { configStore } from "./lib/stores/configStore";
import { sysStore } from "./lib/stores/sysStore";
import TopBar from "./components/TopBar";
import NavBar from "./components/NavBar";
import Toast from "./components/Toast";

const routes = [
  { id: "status", component: lazy(() => import("./routes/StatusTab")) },
  { id: "config", component: lazy(() => import("./routes/ConfigTab")) },
  { id: "modules", component: lazy(() => import("./routes/ModulesTab")) },
  { id: "info", component: lazy(() => import("./routes/InfoTab")) },
];

export default function App() {
  const [activeTab, setActiveTab] = createSignal("status");
  const [dragOffset, setDragOffset] = createSignal(0);
  const [isDragging, setIsDragging] = createSignal(false);

  let containerRef: HTMLDivElement | undefined;
  let containerWidth = 0;
  let touchStartX = 0;
  let touchStartY = 0;
  let ticking = false;
  let rafId: number | null = null;

  const visibleTabs = createMemo(() => routes.map((r) => r.id));

  const baseTranslateX = createMemo(() => {
    const index = visibleTabs().indexOf(activeTab());
    return index * -(100 / visibleTabs().length);
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

  onMount(async () => {
    await uiStore.init();
    await Promise.all([configStore.loadConfig(), sysStore.loadStatus()]);
  });

  return (
    <div class="app-root">
      <Show
        when={uiStore.isReady}
        fallback={
          <div class="loading-container">
            <div class="spinner"></div>
            <span class="loading-text">Loading...</span>
          </div>
        }
      >
        <TopBar />
        <main
          class="main-content"
          ref={containerRef}
          onTouchStart={handleTouchStart}
          onTouchMove={handleTouchMove}
          onTouchEnd={handleTouchEnd}
          onTouchCancel={handleTouchEnd}
        >
          <div
            class="swipe-track"
            style={{
              transform: `translateX(calc(${baseTranslateX()}% + ${dragOffset()}px))`,
              width: `${visibleTabs().length * 100}%`,
              transition: isDragging()
                ? "none"
                : "transform 0.4s cubic-bezier(0.2, 1, 0.2, 1)",
            }}
          >
            <For each={routes}>
              {(route) => (
                <div
                  class="swipe-page"
                  style={{ width: `${100 / visibleTabs().length}%` }}
                >
                  <div class="page-scroller">
                    <route.component />
                  </div>
                </div>
              )}
            </For>
          </div>
        </main>
        <NavBar
          activeTab={activeTab()}
          onTabChange={setActiveTab}
          tabs={routes}
        />
      </Show>
      <Toast />
    </div>
  );
}
