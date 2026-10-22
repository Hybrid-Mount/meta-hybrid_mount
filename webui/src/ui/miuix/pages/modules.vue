<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  MiuixSearchBar,
  MiuixCard,
  MiuixText,
  MiuixBasicComponent,
  MiuixProgressIndicator,
  MiuixIcon,
  MiuixIconButton,
  IconCheck,
} from "miuix-vue";
import { Delete, ExpandLess, ExpandMore, Refresh } from "miuix-vue/icons";

import MiuixRuntimeControls from "../components/MiuixRuntimeControls.vue";
import { runtimeStore } from "../../../lib/stores/runtimeStore";
import { moduleStore } from "../../../lib/stores/moduleStore";
import { uiStore } from "../../../lib/stores/uiStore";
import { sysStore } from "../../../lib/stores/sysStore";
import { matchesModuleFilter, type ModuleFilter } from "../../../lib/moduleFilter";
import type { Module, ModuleRule, MountMode } from "../../../lib/types";
import MiuixSelectField, {
  type MiuixSelectOption,
} from "../components/MiuixSelectField.vue";

const { t } = useI18n();

const searchQuery = ref("");
const filter = ref<ModuleFilter>("active");
const modeOptions = computed(() => sysStore.mountModes);
const modeLabels = computed<Record<MountMode, string>>(() => ({
  overlay: t("config.modeOverlay"),
  magic: t("config.modeMagic"),
  vfs: t("config.modeVfs"),
  ignore: t("config.modeIgnore"),
}));
const filterOptions = computed<MiuixSelectOption[]>(() => [
  { value: "active", label: t("modules.filterActive") },
  { value: "all", label: t("modules.filterAll") },
  ...modeOptions.value.map((mode) => ({
    value: mode,
    label: modeLabels.value[mode],
  })),
]);
const ruleModeOptions = computed<MiuixSelectOption[]>(() =>
  modeOptions.value.map((mode) => ({
    value: mode,
    label: modeLabels.value[mode],
  })),
);
const defaultModeOptions = computed<MiuixSelectOption[]>(() => [
  { value: "", label: t("config.inherit") },
  ...ruleModeOptions.value,
]);

const filteredModules = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  const activeFilter = filter.value;
  return moduleStore.modules.filter((module) => {
    if (!matchesModuleFilter(module, activeFilter)) return false;
    if (!query) return true;
    return (
      module.name.toLowerCase().includes(query) || module.id.toLowerCase().includes(query)
    );
  });
});
const mountErrorCount = computed(
  () => moduleStore.modules.filter((module) => module.mount_error).length,
);

const expanded = ref<Record<string, boolean>>({});
const editingRules = ref<Record<string, ModuleRule>>({});
const savingModule = ref("");

function ruleFor(module: Module): ModuleRule {
  if (!editingRules.value[module.id]) {
    const rule = module.rules;
    editingRules.value[module.id] = {
      default_mode:
        rule.default_mode === "overlay" ||
        rule.default_mode === "magic" ||
        rule.default_mode === "vfs" ||
        rule.default_mode === "ignore"
          ? rule.default_mode
          : null,
      paths: { ...rule.paths } as Record<string, MountMode>,
    };
  }
  return editingRules.value[module.id];
}

async function saveModuleRules(module: Module): Promise<void> {
  savingModule.value = module.id;
  const ok = await moduleStore.saveModuleRules(module.id, ruleFor(module));
  savingModule.value = "";
  uiStore.showToast(ok ? t("modules.saveSuccess") : t("modules.saveFailed"));
}

async function clearErrors(): Promise<void> {
  const removed = await sysStore.clearMountErrors();
  uiStore.showToast(t("modules.clearedCount", { count: removed }));
}

function modeLabel(mode: MountMode): string {
  return modeLabels.value[mode];
}

function toggleModule(moduleId: string): void {
  expanded.value[moduleId] = !expanded.value[moduleId];
}

function enableModuleDetails(element: Element): void {
  element.removeAttribute("aria-hidden");
  element.removeAttribute("inert");
}

function disableModuleDetails(element: Element): void {
  element.setAttribute("aria-hidden", "true");
  element.setAttribute("inert", "");
}

/* Pull to refresh. One slot in the toolbar's old loading position serves both
 * states: it tracks the finger while dragging, then holds the spinner at its
 * resting height until the refresh finishes, so nothing second pops in on release. */
const PULL_MAX = 96;
const PULL_TRIGGER = 56;
const PULL_RATIO = 0.5;
const REFRESH_SLOT = 44;

const pageRef = ref<HTMLElement | null>(null);
const pullDistance = ref(0);
const pullDragging = ref(false);
const pullReady = computed(() => pullDistance.value >= PULL_TRIGGER);
const refreshSlot = computed(() =>
  moduleStore.loading ? Math.max(REFRESH_SLOT, pullDistance.value) : pullDistance.value,
);
const spinnerOpacity = computed(() =>
  moduleStore.loading ? 1 : Math.min(1, pullDistance.value / PULL_TRIGGER),
);

let pullStartX = 0;
let pullStartY = 0;

function scrollViewport(): HTMLElement | null {
  return pageRef.value?.closest<HTMLElement>(".m-scroll-area__viewport") ?? null;
}

function onPullStart(event: TouchEvent): void {
  if (event.touches.length !== 1 || runtimeStore.loading || runtimeStore.busy) return;
  const viewport = scrollViewport();
  if (viewport && viewport.scrollTop > 0) return;
  pullStartX = event.touches[0].clientX;
  pullStartY = event.touches[0].clientY;
  pullDragging.value = true;
}

function onPullMove(event: TouchEvent): void {
  if (!pullDragging.value || event.touches.length !== 1) return;
  const deltaX = event.touches[0].clientX - pullStartX;
  const deltaY = event.touches[0].clientY - pullStartY;
  if (deltaY <= 0 || Math.abs(deltaX) > Math.abs(deltaY)) {
    // Upward, or sideways enough that the pager should own it.
    pullDragging.value = false;
    pullDistance.value = 0;
    return;
  }
  // Own the gesture so the WebView neither scrolls nor rubber-bands behind it.
  if (event.cancelable) event.preventDefault();
  pullDistance.value = Math.min(PULL_MAX, deltaY * PULL_RATIO);
}

function onPullEnd(): void {
  if (!pullDragging.value) return;
  pullDragging.value = false;
  const triggered = pullDistance.value >= PULL_TRIGGER;
  pullDistance.value = 0;
  if (triggered) void runtimeStore.refresh();
}

onMounted(() =>
  Promise.all([
    sysStore.ensureStatusLoaded(),
    moduleStore.ensureModulesLoaded(),
    runtimeStore.loadRuntimeStatus(),
  ]),
);
</script>

<template>
  <div
    ref="pageRef"
    class="page"
    @touchstart.passive="onPullStart"
    @touchmove="onPullMove"
    @touchend="onPullEnd"
    @touchcancel="onPullEnd"
  >
    <div class="modules-toolbar">
      <MiuixSearchBar
        v-model="searchQuery"
        class="module-search"
        :placeholder="t('modules.searchPlaceholder')"
      />
      <MiuixSelectField
        compact
        class="filter-select"
        :label="t('modules.filterLabel')"
        :model-value="filter"
        :options="filterOptions"
        @update:model-value="filter = $event as ModuleFilter"
      />
      <MiuixIconButton
        class="module-icon-action refresh-modules-button"
        :title="t('modules.reload')"
        :aria-label="t('modules.reload')"
        :disabled="runtimeStore.busy || runtimeStore.loading"
        @click="runtimeStore.refresh()"
      >
        <MiuixIcon :icon="Refresh" :size="22" />
      </MiuixIconButton>
    </div>
    <div
      class="modules-refresh"
      :class="{ 'modules-refresh--dragging': pullDragging }"
      :style="{ height: `${refreshSlot}px` }"
      role="status"
      :aria-busy="moduleStore.loading"
    >
      <span class="modules-refresh__spinner" :style="{ opacity: spinnerOpacity }">
        <MiuixProgressIndicator
		  v-if="!moduleStore.loading"
          type="infinite"
          :size="20"
          :color="
            pullReady
              ? 'var(--m-color-primary)'
              : 'var(--m-color-on-background-variant)'
          "
        />
		<MiuixProgressIndicator
		  v-else
		  type="infinite"
		  :size="20"
		  color="var(--m-color-on-background-variant)"
		/>
      </span>
    </div>

    <MiuixCard v-if="mountErrorCount" class="card module-error-card">
      <div class="module-error-notice">
        <span class="module-error-copy">
          <strong>{{ t("modules.mountError") }}</strong>
          <span>{{ t("modules.mountErrorSummary", { count: mountErrorCount }) }}</span>
        </span>
        <MiuixIconButton
          class="module-icon-action clear-errors-button"
          :title="t('modules.clearErrors')"
          :aria-label="t('modules.clearErrors')"
          @click="clearErrors"
        >
          <MiuixIcon :icon="Delete" :size="22" />
        </MiuixIconButton>
      </div>
    </MiuixCard>

    <template v-for="module in filteredModules" :key="module.id">
      <MiuixCard class="card">
        <MiuixBasicComponent
          class="module-header"
          :title="module.name || module.id"
          :summary="`${module.id} · ${module.version || '-'} · ${module.author}`"
          clickable
          role="button"
          tabindex="0"
          :aria-expanded="Boolean(expanded[module.id])"
          @click="toggleModule(module.id)"
          @keydown.enter="toggleModule(module.id)"
          @keydown.space.prevent="toggleModule(module.id)"
        >
          <template #end>
            <span class="module-header-end">
              <MiuixText
                v-if="
                  module.blacklisted || module.mode !== 'vfs' || sysStore.vfsSupported
                "
                :color="
                  module.blacklisted || module.mode === 'ignore' ? 'error' : 'success'
                "
              >
                {{
                  module.blacklisted ? t("modules.blacklisted") : modeLabel(module.mode)
                }}
              </MiuixText>
              <MiuixIcon
                :icon="expanded[module.id] ? ExpandLess : ExpandMore"
                :size="20"
                aria-hidden="true"
              />
            </span>
          </template>
        </MiuixBasicComponent>

        <p v-if="module.blacklisted" class="module-blacklist-notice">
          {{ t("modules.blacklistReason") }}
        </p>

        <Transition
          name="module-details"
          @before-enter="enableModuleDetails"
          @before-leave="disableModuleDetails"
        >
          <div v-if="expanded[module.id]" class="module-details-wrapper">
            <div class="module-details-inner">
              <div class="module-details">
                <MiuixRuntimeControls :module-id="module.id" />
                <MiuixBasicComponent
                  v-if="module.mount_error"
                  :title="t('modules.mountError')"
                  :summary="module.mount_error"
                />
                <MiuixBasicComponent
                  v-if="module.suggest_ignore"
                  :title="t('modules.suggestIgnore')"
                />
                <div class="rule-row">
                  <span>{{ t("config.moduleDefault") }}</span>
                  <MiuixSelectField
                    compact
                    class="rule-mode-select"
                    :label="t('config.moduleDefault')"
                    :model-value="ruleFor(module).default_mode ?? ''"
                    :options="defaultModeOptions"
                    @update:model-value="
                      ruleFor(module).default_mode = $event ? ($event as MountMode) : null
                    "
                  />
                </div>
                <div
                  v-for="(mode, path) in ruleFor(module).paths"
                  :key="path"
                  class="rule-row"
                >
                  <span class="path">{{ path }}</span>
                  <MiuixSelectField
                    compact
                    class="rule-mode-select"
                    :label="String(path)"
                    :model-value="mode"
                    :options="ruleModeOptions"
                    @update:model-value="
                      ruleFor(module).paths[path] = $event as MountMode
                    "
                  />
                </div>
                <div class="module-actions">
                  <MiuixIconButton
                    class="module-icon-action save-module-button"
                    :title="t('modules.save')"
                    :aria-label="t('modules.save')"
                    :disabled="savingModule === module.id"
                    @click="saveModuleRules(module)"
                  >
                    <IconCheck class="check-icon" />
                  </MiuixIconButton>
                </div>
              </div>
            </div>
          </div>
        </Transition>
      </MiuixCard>
    </template>

    <MiuixBasicComponent
      v-if="!moduleStore.loading && filteredModules.length === 0"
      :title="t('modules.empty')"
    />
  </div>
</template>

<style scoped>
/* Pull to refresh. The spinner is pinned to the top of the slot, so it stays put
 * right under the toolbar while the cards slide away from it; the drag drives the
 * height directly and only the release settles. */
.modules-refresh {
  height: 0;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  overflow: hidden;
  transition: height 300ms cubic-bezier(0.2, 0.9, 0.2, 1);
}

.modules-refresh--dragging {
  transition: none;
}

.modules-refresh__spinner {
  display: flex;
  /* Same 12px band the loading row used before it became a pull slot. */
  padding-top: 12px;
}

.module-header {
  cursor: pointer;
  border-radius: var(--m-radius-md, 16px);
}

.module-header :deep(.m-basic-component__row) {
  width: 100%;
  min-width: 0;
}

.modules-toolbar {
  width: auto;
  margin-inline: 12px;
  margin-bottom: 12px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(140px, 190px) 40px;
  gap: 8px;
  align-items: center;
}

.module-search,
.filter-select {
  min-width: 0;
}

.module-search :deep(.m-search-bar__row) {
  padding-inline: 0;
}

.module-error-notice {
  min-height: 48px;
  padding: 8px 8px 8px 16px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.module-error-copy {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  text-align: left;
}

.module-error-copy strong {
  color: var(--m-color-error, #ba1a1a);
}

.module-error-copy span {
  color: var(--m-color-on-surface-variant-summary);
  font-size: 13px;
}

.module-header:focus-visible {
  outline: 2px solid var(--m-color-primary, #6750a4);
  outline-offset: -2px;
}

.module-header-end {
  display: flex;
  align-items: center;
  gap: 8px;
}

.module-blacklist-notice {
  margin: 0 16px 12px;
  padding: 8px 10px;
  border-radius: var(--m-radius-xs, 8px);
  background: var(--m-color-error-container, rgba(186, 26, 26, 0.12));
  color: var(--m-color-on-error-container, #410002);
  font-size: 13px;
  line-height: 1.5;
}

/* Expanded body sits inside the card. Rows keep their own 16px inset (miuix
 * BasicComponent inside margin), so this wrapper adds no horizontal padding —
 * it only draws the separator under the header row. */
.module-details {
  padding: 0;
  border-top: 1px solid var(--m-color-outline-variant, rgba(0, 0, 0, 0.08));
}

.module-details-wrapper {
  display: grid;
  grid-template-rows: 1fr;
}

.module-details-inner {
  min-height: 0;
  overflow: hidden;
}

.rule-row {
  display: flex;
  gap: 12px;
  align-items: center;
  min-height: 56px;
  padding: 0 16px;
}

.rule-row .path {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rule-mode-select {
  margin-left: auto;
}

.module-actions {
  display: flex;
  justify-content: flex-end;
  padding: 0 16px 12px;
}

.module-icon-action {
  color: var(--m-color-on-surface-variant-summary);
}

.save-module-button {
  --m-icon-button-bg: var(--m-color-primary, #6750a4);
  color: var(--m-color-on-primary, #fff);
}

.refresh-modules-button {
  --m-icon-button-bg: var(--m-color-surface-container-high, rgba(0, 0, 0, 0.06));
  --m-icon-button-radius: var(--m-radius-sm, 12px);
  color: var(--m-color-on-surface-variant-summary);
}

.refresh-modules-button:disabled {
  opacity: 0.45;
}

.clear-errors-button {
  --m-icon-button-bg: var(--m-color-error-container, rgba(186, 26, 26, 0.12));
  color: var(--m-color-on-error-container, #410002);
  flex: 0 0 auto;
}

.check-icon {
  width: 22px;
  height: 22px;
  color: currentColor;
}

.module-details-enter-active,
.module-details-leave-active {
  transition: grid-template-rows 280ms cubic-bezier(0.2, 0, 0, 1);
}

.module-details-enter-active .module-details,
.module-details-leave-active .module-details {
  transition:
    opacity 180ms ease,
    transform 180ms ease;
}

.module-details-enter-from,
.module-details-leave-to {
  grid-template-rows: 0fr;
}

.module-details-enter-from .module-details,
.module-details-leave-to .module-details {
  opacity: 0;
  transform: translateY(-6px);
}

@media (prefers-reduced-motion: reduce) {
  .module-details-enter-active,
  .module-details-leave-active,
  .module-details-enter-active .module-details,
  .module-details-leave-active .module-details {
    transition-duration: 0.01ms;
  }
}

@media (max-width: 560px) {
  .modules-toolbar {
    grid-template-columns: minmax(0, 1fr) minmax(104px, 30vw) 40px;
    gap: 8px;
  }
}

@media (max-width: 420px) {
  .module-header :deep(.m-basic-component__row) {
    flex-wrap: wrap;
    row-gap: 8px;
  }

  .module-header :deep(.m-basic-component__center) {
    flex-basis: 100%;
  }

  .module-header :deep(.m-basic-component__end) {
    margin-left: auto;
  }
}
</style>
