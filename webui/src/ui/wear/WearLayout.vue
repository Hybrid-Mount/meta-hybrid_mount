<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import "../md3/material";
import "../md3/theme.css";
import "./wear.css";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { sysStore } from "../../lib/stores/sysStore";
import { moduleStore } from "../../lib/stores/moduleStore";
import { uiStore } from "../../lib/stores/uiStore";
import { backendDisplayKeys, collectStatusErrors } from "../../lib/statusMounts";
import { ICONS } from "../md3/icons";

const props = defineProps<{ pages: Component[]; titles: string[] }>();
const { t } = useI18n();
const activePage = ref<number | null>(null);
const visitedPages = ref(new Set<number>());
const scroller = ref<HTMLElement | null>(null);
const title = ref<HTMLElement | null>(null);
let menuScrollTop = 0;
let menuTrigger: HTMLElement | null = null;
const previewToast = ref("");
let previewToastTimer = 0;
const icons = [ICONS.storage, ICONS.settings, ICONS.modules, ICONS.info];
const backend = computed(() =>
  backendDisplayKeys(sysStore.state, sysStore.vfsSupported)
    .map((key) => t(key))
    .join(" · "),
);
const mounted = computed(
  () => moduleStore.modules.filter((module) => module.is_mounted).length,
);
const showMountedCount = computed(() => !moduleStore.loading && moduleStore.hasLoaded);
const hasErrors = computed(
  () =>
    collectStatusErrors(sysStore.state, {
      loadError: sysStore.loadError,
      moduleScanError: moduleStore.loadError,
    }).length > 0,
);

async function openPage(page: number, event: MouseEvent): Promise<void> {
  menuScrollTop = scroller.value?.scrollTop ?? 0;
  menuTrigger = event.currentTarget as HTMLElement;
  visitedPages.value.add(page);
  activePage.value = page;
  await nextTick();
  scroller.value?.scrollTo(0, 0);
  title.value?.focus({ preventScroll: true });
}

async function returnToMenu(): Promise<void> {
  activePage.value = null;
  await nextTick();
  scroller.value?.scrollTo(0, menuScrollTop);
  // The menu is remounted when returning from a detail screen.
  const trigger = menuTrigger?.id ? document.getElementById(menuTrigger.id) : null;
  (trigger ?? title.value)?.focus({ preventScroll: true });
  menuTrigger = null;
}

onMounted(() => {
  document.documentElement.classList.add("md3-active");
  if (import.meta.env.DEV) {
    uiStore.setToastHandler((message) => {
      previewToast.value = message;
      window.clearTimeout(previewToastTimer);
      previewToastTimer = window.setTimeout(() => (previewToast.value = ""), 2600);
    });
  }
  void sysStore.ensureStatusLoaded();
  void moduleStore.ensureModulesLoaded();
});

onBeforeUnmount(() => {
  document.documentElement.classList.remove("md3-active");
  if (import.meta.env.DEV) {
    window.clearTimeout(previewToastTimer);
    uiStore.setToastHandler();
  }
});
</script>

<template>
  <div class="wear-root">
    <main ref="scroller" class="page-scroller wear-scroll">
      <button
        v-if="activePage !== null"
        type="button"
        class="wear-home"
        @click="returnToMenu"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.42-1.41L7.83 13H20v-2z" />
        </svg>
        {{ t("common.appName") }}
      </button>
      <h1 ref="title" class="wear-title" tabindex="-1">
        {{ activePage === null ? t("common.appName") : props.titles[activePage] }}
      </h1>
      <template v-if="activePage === null">
        <div v-if="backend || showMountedCount" class="wear-summary">
          <strong v-if="backend">{{ backend }}</strong>
          <span v-if="showMountedCount">{{
            t("status.moduleCount", { count: mounted })
          }}</span>
        </div>
        <nav class="wear-menu" :aria-label="t('common.appName')">
          <button
            v-for="(label, index) in props.titles"
            :key="index"
            type="button"
            class="wear-button"
            :id="`wear-destination-${index}`"
            @click="openPage(index, $event)"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true"><path :d="icons[index]" /></svg>
            <span>{{ label }}</span>
            <svg class="wear-chevron" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M9 18l6-6-6-6-1.41 1.41L12.17 12l-4.58 4.59L9 18z" />
            </svg>
          </button>
        </nav>
        <button
          v-if="hasErrors"
          type="button"
          class="wear-button wear-error"
          id="wear-status-error"
          @click="openPage(0, $event)"
        >
          {{ t("common.statusErrors") }}
        </button>
      </template>
      <div
        v-for="(page, index) in props.pages"
        v-show="index === activePage"
        :key="index"
        class="wear-page"
      >
        <component :is="page" v-if="visitedPages.has(index)" />
      </div>
      <button
        v-if="activePage !== null"
        type="button"
        class="wear-button"
        @click="returnToMenu"
      >
        {{ t("common.appName") }}
      </button>
    </main>
    <div v-if="previewToast" class="md3-toast wear-preview-toast" role="status">
      {{ previewToast }}
    </div>
  </div>
</template>
