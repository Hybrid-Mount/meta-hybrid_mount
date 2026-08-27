<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import "./material";
import "./theme.css";
import { onBeforeUnmount, onMounted, ref, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { uiStore } from "../../lib/stores/uiStore";
import { sysStore } from "../../lib/stores/sysStore";
import { useSwipePager } from "../useSwipePager";
import { ICONS } from "./icons";

const { t } = useI18n();

const props = defineProps<{
  navindex: number;
  pages: Component[];
  titles: string[];
}>();

const emit = defineEmits<{
  (event: "update:navindex", value: number): void;
}>();

const rebootOpen = ref(false);
const toastText = ref("");
const navIconKeys = ["home", "settings", "modules", "info"] as const;
let toastTimer = 0;

function showToast(text: string): void {
  toastText.value = text;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toastText.value = "";
  }, 2600);
}

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
  onTouchStart,
  onTouchMove,
  onTouchEnd,
  onTouchCancel,
} = useSwipePager(
  () => props.navindex,
  () => props.pages.length,
  selectTab,
  swipeContainerRef,
);

function iconPath(index: number): string {
  const key = navIconKeys[index];
  const activeKey = `${key}_filled` as keyof typeof ICONS;
  return props.navindex === index ? ICONS[activeKey] || ICONS[key] : ICONS[key];
}

async function rebootSystem(): Promise<void> {
  rebootOpen.value = false;
  await sysStore.rebootDevice();
}

onMounted(() => {
  document.documentElement.classList.add("md3-active");
  uiStore.setToastHandler(showToast);
});

onBeforeUnmount(() => {
  document.documentElement.classList.remove("md3-active");
  uiStore.setToastHandler();
  window.clearTimeout(toastTimer);
});
</script>

<template>
  <div class="app-root">
    <header class="top-bar">
      <div class="top-bar-content">
        <h1 class="screen-title">{{ t("common.appName") }}</h1>
      </div>
    </header>

    <main
      ref="swipeContainerRef"
      class="main-content"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerCancel"
      @touchstart="onTouchStart"
      @touchmove="onTouchMove"
      @touchend="onTouchEnd"
      @touchcancel="onTouchCancel"
    >
      <div class="swipe-track" :class="{ 'is-dragging': isDragging }" :style="trackStyle">
        <div
          v-for="(page, index) in pages"
          :key="index"
          class="swipe-page"
          :aria-hidden="navindex !== index"
          :inert="navindex !== index"
        >
          <div class="page-scroller">
            <component :is="page" v-if="visitedPages.has(index)" />
          </div>
        </div>
      </div>
    </main>

    <nav class="bottom-nav" :aria-label="t('common.appName')">
      <button
        v-for="(title, index) in titles"
        :key="title"
        type="button"
        class="nav-tab"
        :class="{ active: navindex === index }"
        :aria-current="navindex === index ? 'page' : undefined"
        @click="selectTab(index)"
      >
        <span class="icon-container">
          <md-icon>
            <svg viewBox="0 0 24 24"><path :d="iconPath(index)" /></svg>
          </md-icon>
        </span>
        <span class="label">{{ title }}</span>
      </button>
    </nav>

    <md-dialog :open="rebootOpen" @closed="rebootOpen = false">
      <div slot="headline">{{ t("common.rebootTitle") }}</div>
      <div slot="content">{{ t("common.rebootConfirm") }}</div>
      <div slot="actions">
        <md-text-button @click="rebootOpen = false">
          {{ t("common.cancel") }}
        </md-text-button>
        <md-text-button @click="rebootSystem">
          {{ t("common.reboot") }}
        </md-text-button>
      </div>
    </md-dialog>

    <Transition name="toast">
      <div v-if="toastText" class="md3-toast" role="status">{{ toastText }}</div>
    </Transition>
  </div>
</template>
