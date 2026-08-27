<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import "miuix-vue/style.css";
import "./theme.css";
import { ref, onMounted, onBeforeUnmount, type Component } from "vue";
import { useI18n } from "vue-i18n";
import {
  MiuixSnackbarHost,
  MiuixScrollArea,
  MiuixIcon,
  MiuixNavigationBar,
  MiuixTopAppBar,
  MiuixIconButton,
  MiuixButton,
  MiuixDialog,
} from "miuix-vue";
import { ScreenMirroring, Settings, Info, Folder, Reset } from "miuix-vue/icons";
import { sysStore } from "../../lib/stores/sysStore";
import { useSwipePager } from "../useSwipePager";

const { t } = useI18n();

const props = defineProps<{
  navindex: number;
  pages: Component[];
  titles: string[];
}>();

const emit = defineEmits<{
  (e: "update:navindex", value: number): void;
}>();

const rebootRequested = ref(false);
const navIcons = [ScreenMirroring, Settings, Folder, Info];

function selectTab(index: number): void {
  if (index === props.navindex) return;
  emit("update:navindex", index);
}

const swipeContainerRef = ref<HTMLElement | null>(null);
const {
  isDragging,
  trackStyle,
  visitedPages,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onPointerCancel,
} = useSwipePager(
  () => props.navindex,
  () => props.pages.length,
  selectTab,
  swipeContainerRef,
);

function rebootSystem(): void {
  sysStore.rebootDevice();
  rebootRequested.value = false;
}

const bottomBarRef = ref<HTMLElement | null>(null);
let barObserver: ResizeObserver | null = null;

function syncSnackbarInset(): void {
  const height = bottomBarRef.value?.offsetHeight ?? 0;
  document.documentElement.style.setProperty("--m-snackbar-inset-bottom", `${height}px`);
}

onMounted(() => {
  if (bottomBarRef.value) {
    barObserver = new ResizeObserver(syncSnackbarInset);
    barObserver.observe(bottomBarRef.value);
  }
  syncSnackbarInset();
});

onBeforeUnmount(() => {
  barObserver?.disconnect();
  document.documentElement.style.removeProperty("--m-snackbar-inset-bottom");
});
</script>

<template>
  <div class="app">
    <MiuixTopAppBar :large="false" :title="t('common.appName')" class="app__top-app-bar">
      <template #actions>
        <MiuixIconButton :aria-label="t('common.reboot')" @click="rebootRequested = true">
          <MiuixIcon :icon="Reset" :size="24" />
        </MiuixIconButton>
      </template>
    </MiuixTopAppBar>

    <main
      ref="swipeContainerRef"
      class="app__body"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerCancel"
    >
      <div
        class="miuix-swipe-track"
        :class="{ 'is-dragging': isDragging }"
        :style="trackStyle"
      >
        <div
          v-for="(page, index) in pages"
          :key="index"
          class="miuix-swipe-page"
          :aria-hidden="navindex !== index"
          :inert="navindex !== index"
        >
          <MiuixScrollArea class="app__page-scroller">
            <component :is="page" v-if="visitedPages.has(index)" />
          </MiuixScrollArea>
        </div>
      </div>
    </main>

    <div ref="bottomBarRef" class="app__bottom">
      <MiuixNavigationBar
        :model-value="navindex"
        :items="titles.map((label) => ({ label }))"
        @update:model-value="selectTab"
      >
        <template #icon="{ index }">
          <MiuixIcon :icon="navIcons[index]" :size="26" />
        </template>
      </MiuixNavigationBar>
    </div>
  </div>

  <MiuixSnackbarHost />

  <MiuixDialog
    v-model="rebootRequested"
    :title="t('common.rebootTitle')"
    :summary="t('common.rebootConfirm')"
    @close="rebootRequested = false"
  >
    <template #default="{ close }">
      <div class="dialog-actions">
        <MiuixButton class="grow" @click="close">
          {{ t("common.cancel") }}
        </MiuixButton>
        <MiuixButton class="grow" type="primary" @click="rebootSystem">
          {{ t("common.reboot") }}
        </MiuixButton>
      </div>
    </template>
  </MiuixDialog>
</template>

<style>
:root {
  --top-inset: var(--window-inset-top, 0px);
  --bottom-inset: var(--window-inset-bottom, 0px);
}

.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--m-color-surface);
}

.app__body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  touch-action: pan-y;
}

.app__top-app-bar {
  flex: none;
  padding-top: var(--top-inset);
}

.miuix-swipe-track {
  display: flex;
  width: calc(var(--swipe-tab-count) * 100%);
  height: 100%;
  will-change: transform;
  transform: translateX(calc(var(--swipe-base-translate) + var(--swipe-drag-offset)));
  transition: transform 0.4s cubic-bezier(0.2, 1, 0.2, 1);
}

.miuix-swipe-track.is-dragging {
  transition: none;
}

.miuix-swipe-page {
  width: calc(100% / var(--swipe-tab-count));
  height: 100%;
  flex: none;
  overflow: hidden;
}

.app__page-scroller {
  width: 100%;
  height: 100%;
}

.app__bottom {
  flex: none;
  z-index: 10;
  padding-bottom: var(--bottom-inset);
}

.m-snackbar-host {
  bottom: calc(var(--m-snackbar-inset-bottom, 0px) + 108px);
}

.m-snackbar {
  width: calc(100% - 24px);
  max-width: 420px;
  padding: 8px 0 0;
}

.m-snackbar__inner {
  min-height: 52px;
  border: 1px solid var(--m-color-outline-variant, rgba(0, 0, 0, 0.12));
  border-radius: 18px;
  padding: 13px 16px;
  background: var(--m-color-surface-container-highest, #e6e0e9);
  color: var(--m-color-on-surface, #1d1b20);
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.2);
}

.m-snackbar__message {
  font-size: 14px;
  font-weight: 500;
  line-height: 20px;
}

@media (prefers-reduced-motion: reduce) {
  .miuix-swipe-track {
    transition-duration: 0.01ms;
  }
}

.dialog-actions {
  display: flex;
  gap: 12px;
}
</style>
