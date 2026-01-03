/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createMemo, createEffect, onMount, Show } from 'solid-js';
import { store } from './lib/store';
import TopBar from './components/TopBar.tsx';
import NavBar from './components/NavBar.tsx';
import Toast from './components/Toast.tsx';
import StatusTab from './routes/StatusTab.tsx';
import ConfigTab from './routes/ConfigTab.tsx';
import ModulesTab from './routes/ModulesTab.tsx';
import LogsTab from './routes/LogsTab.tsx';
import InfoTab from './routes/InfoTab.tsx';
import GranaryTab from './routes/GranaryTab.tsx';
import WinnowingTab from './routes/WinnowingTab.tsx';

export default function App() {
  const [activeTab, setActiveTab] = createSignal('status');
  const [dragOffset, setDragOffset] = createSignal(0);
  const [isDragging, setIsDragging] = createSignal(false);
  const [isReady, setIsReady] = createSignal(false);
  
  let containerRef: HTMLDivElement | undefined;
  let containerWidth = 0;
  
  let touchStartX = 0;
  let touchStartY = 0;

  const visibleTabs = createMemo(() => {
    const tabs = ['status', 'config', 'modules', 'logs', 'granary'];
    if (store.conflicts.length > 0) {
      tabs.push('winnowing');
    }
    tabs.push('info');
    return tabs;
  });

  const baseTranslateX = createMemo(() => {
    const index = visibleTabs().indexOf(activeTab());
    return index * -(100 / visibleTabs().length);
  });

  function switchTab(id: string) {
    setActiveTab(id);
  }

  function handleTouchStart(e: TouchEvent) {
    touchStartX = e.changedTouches[0].screenX;
    touchStartY = e.changedTouches[0].screenY;
    setIsDragging(true);
    setDragOffset(0);
  }

  function handleTouchMove(e: TouchEvent) {
    if (!isDragging()) return;
    const currentX = e.changedTouches[0].screenX;
    const currentY = e.changedTouches[0].screenY;
    let diffX = currentX - touchStartX;
    const diffY = currentY - touchStartY;

    if (Math.abs(diffY) > Math.abs(diffX)) return;
    
    if (e.cancelable) e.preventDefault();

    const tabs = visibleTabs();
    const currentIndex = tabs.indexOf(activeTab());

    if ((currentIndex === 0 && diffX > 0) || (currentIndex === tabs.length - 1 && diffX < 0)) {
      diffX = diffX / 3;
    }
    setDragOffset(diffX);
  }

  function handleTouchEnd() {
    if (!isDragging()) return;
    setIsDragging(false);
    
    if (containerRef) {
        containerWidth = containerRef.clientWidth;
    }

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

    if (nextIndex !== currentIndex) {
      switchTab(tabs[nextIndex]);
    }
    setDragOffset(0);
  }

  createEffect(() => {
    if (activeTab() === 'winnowing' && !visibleTabs().includes('winnowing')) {
      setActiveTab('granary');
    }
  });

  onMount(async () => {
    try {
      await store.init();
    } finally {
      setIsReady(true);
    }
  });

  return (
    <div class="app-root">
      <Show when={isReady()} fallback={
        <div class="loading-container">
           <div class="spinner"></div>
           <span class="loading-text">Loading...</span>
        </div>
      }>
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
                transition: isDragging() ? 'none' : 'transform 0.4s cubic-bezier(0.2, 1, 0.2, 1)'
            }}
          >
            <div class="swipe-page" style={{ width: `${100 / visibleTabs().length}%` }}><div class="page-scroller"><StatusTab /></div></div>
            <div class="swipe-page" style={{ width: `${100 / visibleTabs().length}%` }}><div class="page-scroller"><ConfigTab /></div></div>
            <div class="swipe-page" style={{ width: `${100 / visibleTabs().length}%` }}><div class="page-scroller"><ModulesTab /></div></div>
            <div class="swipe-page" style={{ width: `${100 / visibleTabs().length}%` }}><div class="page-scroller"><LogsTab /></div></div>
            <div class="swipe-page" style={{ width: `${100 / visibleTabs().length}%` }}><div class="page-scroller"><GranaryTab /></div></div>
            
            <Show when={store.conflicts.length > 0}>
                <div class="swipe-page" style={{ width: `${100 / visibleTabs().length}%` }}><div class="page-scroller"><WinnowingTab /></div></div>
            </Show>
            
            <div class="swipe-page" style={{ width: `${100 / visibleTabs().length}%` }}><div class="page-scroller"><InfoTab /></div></div>
          </div>
        </main>
        <NavBar activeTab={activeTab()} onTabChange={switchTab} />
      </Show>
      <Toast />
    </div>
  );
}