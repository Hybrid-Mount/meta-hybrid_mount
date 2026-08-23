<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import "./material";
import "./theme.css";
import { onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { uiStore } from "../../lib/stores/uiStore";
import { sysStore } from "../../lib/stores/sysStore";
import { ICONS } from "./icons";

const { t } = useI18n();

const props = defineProps<{
  navindex: number;
  activepage: Component;
  titles: string[];
}>();

const emit = defineEmits<{
  (event: "update:navindex", value: number): void;
}>();

const pageScrollerRef = ref<HTMLElement | null>(null);
const rebootOpen = ref(false);
const toastText = ref("");
const pageTransition = ref("page-forward");
const scrollPositions = new Map<number, number>();
const navIconKeys = ["home", "settings", "modules", "info"] as const;
let toastTimer = 0;
let touchStartX = 0;
let touchStartY = 0;

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

function iconPath(index: number): string {
  const key = navIconKeys[index];
  const activeKey = `${key}_filled` as keyof typeof ICONS;
  return props.navindex === index ? ICONS[activeKey] || ICONS[key] : ICONS[key];
}

function onTouchStart(event: TouchEvent): void {
  touchStartX = event.changedTouches[0]?.screenX ?? 0;
  touchStartY = event.changedTouches[0]?.screenY ?? 0;
}

function onTouchEnd(event: TouchEvent): void {
  const end = event.changedTouches[0];
  if (!end) return;
  const deltaX = end.screenX - touchStartX;
  const deltaY = end.screenY - touchStartY;
  if (Math.abs(deltaX) < 72 || Math.abs(deltaX) < Math.abs(deltaY) * 1.2) return;
  const next = deltaX < 0 ? props.navindex + 1 : props.navindex - 1;
  if (next >= 0 && next < props.titles.length) selectTab(next);
}

async function rebootSystem(): Promise<void> {
  rebootOpen.value = false;
  await sysStore.rebootDevice();
}

watch(
  () => props.navindex,
  (next, previous) => {
    pageTransition.value = next > previous ? "page-forward" : "page-back";
    scrollPositions.set(previous, pageScrollerRef.value?.scrollTop ?? 0);
  },
  { flush: "pre" },
);

function onPageEnter(): void {
  pageScrollerRef.value?.scrollTo({
    top: scrollPositions.get(props.navindex) ?? 0,
    behavior: "instant" as ScrollBehavior,
  });
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
      class="main-content"
      @touchstart.passive="onTouchStart"
      @touchend.passive="onTouchEnd"
    >
      <div class="swipe-page">
        <Transition :name="pageTransition" mode="out-in" @enter="onPageEnter">
          <div ref="pageScrollerRef" :key="navindex" class="page-scroller">
            <KeepAlive>
              <component :is="activepage" v-if="activepage" />
            </KeepAlive>
          </div>
        </Transition>
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
