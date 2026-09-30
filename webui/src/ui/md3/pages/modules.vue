<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import Md3RuntimeControls from "../components/Md3RuntimeControls.vue";
import { runtimeStore } from "../../../lib/stores/runtimeStore";
import { moduleStore } from "../../../lib/stores/moduleStore";
import { sysStore } from "../../../lib/stores/sysStore";
import { uiStore } from "../../../lib/stores/uiStore";
import { matchesModuleFilter, type ModuleFilter } from "../../../lib/moduleFilter";
import type { Module, ModuleRule, MountMode } from "../../../lib/types";
import Md3SelectField, { type SelectOption } from "../components/Md3SelectField.vue";
import { ICONS } from "../icons";

const { t } = useI18n();
const modeOptions = computed(() => sysStore.mountModes);
const modeLabels = computed<Record<MountMode, string>>(() => ({
  overlay: t("config.modeOverlay"),
  magic: t("config.modeMagic"),
  vfs: t("config.modeVfs"),
  ignore: t("config.modeIgnore"),
}));
const modeSelectOptions = computed<SelectOption[]>(() =>
  modeOptions.value.map((mode) => ({ value: mode, label: modeLabels.value[mode] })),
);
const filterOptions = computed<SelectOption[]>(() => [
  { value: "active", label: t("modules.filterActive") },
  { value: "all", label: t("modules.filterAll") },
  ...modeSelectOptions.value,
]);
const query = ref("");
const filter = ref<ModuleFilter>("active");
const expanded = ref<Record<string, boolean>>({});
const editing = ref<Record<string, ModuleRule>>({});
const newPaths = ref<Record<string, string>>({});
const newModes = ref<Record<string, MountMode>>({});

const filtered = computed(() =>
  moduleStore.modules.filter((module) => {
    if (!matchesModuleFilter(module, filter.value)) return false;
    const needle = query.value.trim().toLowerCase();
    if (!needle) return true;
    return [module.name, module.id, module.author].some((value) =>
      value.toLowerCase().includes(needle),
    );
  }),
);
const mountErrorCount = computed(
  () => moduleStore.modules.filter((module) => module.mount_error).length,
);

function ruleFor(module: Module): ModuleRule {
  editing.value[module.id] ??= {
    default_mode: module.rules.default_mode,
    paths: { ...module.rules.paths },
  };
  return editing.value[module.id];
}

function addPath(module: Module): void {
  const path = (newPaths.value[module.id] ?? "").trim().replace(/^\/+/, "");
  if (!path) return;
  ruleFor(module).paths[path] = newModes.value[module.id] ?? "overlay";
  newPaths.value[module.id] = "";
}

async function saveRules(module: Module): Promise<void> {
  const rule = JSON.parse(JSON.stringify(ruleFor(module))) as ModuleRule;
  const ok = await moduleStore.saveModuleRules(module.id, rule);
  uiStore.showToast(ok ? t("modules.saveSuccess") : t("modules.saveFailed"));
}

async function clearErrors(): Promise<void> {
  const removed = await sysStore.clearMountErrors();
  uiStore.showToast(t("modules.clearedCount", { count: removed }));
}

function enableModuleDetails(element: Element): void {
  element.removeAttribute("aria-hidden");
  element.removeAttribute("inert");
}

function disableModuleDetails(element: Element): void {
  element.setAttribute("aria-hidden", "true");
  element.setAttribute("inert", "");
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
  <div class="modules-page">
    <section class="header-section">
      <div class="search-bar">
        <svg class="search-icon" viewBox="0 0 24 24"><path :d="ICONS.search" /></svg>
        <input
          v-model="query"
          class="search-input"
          :placeholder="t('modules.searchPlaceholder')"
        />
        <div class="filter-group">
          <Md3SelectField
            compact
            class="filter-select-field"
            :label="t('modules.filterLabel')"
            :model-value="filter"
            :options="filterOptions"
            @update:model-value="filter = $event as ModuleFilter"
          />
        </div>
      </div>
      <md-filled-tonal-icon-button
        class="refresh-modules-action"
        :title="t('modules.reload')"
        :aria-label="t('modules.reload')"
        :disabled="runtimeStore.busy || runtimeStore.loading"
        @click="runtimeStore.refresh()"
      >
        <md-icon
          ><svg viewBox="0 0 24 24"><path :d="ICONS.refresh" /></svg
        ></md-icon>
      </md-filled-tonal-icon-button>
    </section>

    <section v-if="mountErrorCount" class="error-banner">
      <md-icon class="error-icon"
        ><svg viewBox="0 0 24 24"><path :d="ICONS.bug" /></svg
      ></md-icon>
      <div class="error-content">
        <strong>{{ t("modules.mountError") }}</strong>
        <span>{{ t("modules.mountErrorSummary", { count: mountErrorCount }) }}</span>
      </div>
      <md-filled-tonal-icon-button
        class="module-icon-action clear-errors-action"
        :title="t('modules.clearErrors')"
        :aria-label="t('modules.clearErrors')"
        @click="clearErrors"
      >
        <md-icon
          ><svg viewBox="0 0 24 24"><path :d="ICONS.delete" /></svg
        ></md-icon>
      </md-filled-tonal-icon-button>
    </section>

    <section class="modules-list">
      <article
        v-for="module in filtered"
        :key="module.id"
        class="module-card"
        :class="{
          expanded: expanded[module.id],
          unmounted: !module.is_mounted,
          blacklisted: module.blacklisted,
          'has-error': Boolean(module.mount_error),
        }"
      >
        <button
          type="button"
          class="module-header"
          :aria-expanded="Boolean(expanded[module.id])"
          @click="expanded[module.id] = !expanded[module.id]"
        >
          <span
            class="mode-indicator"
            :class="`mode-${module.blacklisted ? 'blacklisted' : module.mode}`"
          />
          <span class="module-info">
            <span class="module-name">{{ module.name || module.id }}</span>
            <span v-if="expanded[module.id]" class="module-id">{{ module.id }}</span>
            <span class="module-meta">
              <span class="version-badge">{{ module.version || "-" }}</span>
              <span>{{ module.author || t("modules.unknownLabel") }}</span>
            </span>
          </span>
          <span
            v-if="module.blacklisted || module.mode !== 'vfs' || sysStore.vfsSupported"
            class="mode-pill"
          >
            {{ module.blacklisted ? t("modules.blacklisted") : modeLabels[module.mode] }}
          </span>
        </button>

        <p v-if="module.blacklisted" class="blacklist-hint module-blacklist-notice">
          {{ t("modules.blacklistReason") }}
        </p>

        <Transition
          name="module-expand"
          @before-enter="enableModuleDetails"
          @before-leave="disableModuleDetails"
        >
          <div v-if="expanded[module.id]" class="module-body-wrapper">
            <div class="module-body-inner">
              <div class="module-body-content">
                <Md3RuntimeControls :module-id="module.id" />
                <section
                  v-if="module.mount_error || module.suggest_ignore"
                  class="body-section"
                >
                  <p v-if="module.mount_error" class="status-warning">
                    {{ t("modules.mountError") }}: {{ module.mount_error }}
                  </p>
                  <p v-if="module.suggest_ignore" class="suggest-ignore-hint">
                    {{ t("modules.suggestIgnore") }}
                  </p>
                </section>

                <section class="body-section">
                  <span class="section-label">{{ t("config.moduleDefault") }}</span>
                  <div class="strategy-selector">
                    <button
                      type="button"
                      class="strategy-option"
                      :class="{ selected: ruleFor(module).default_mode === null }"
                      @click="ruleFor(module).default_mode = null"
                    >
                      <span class="opt-title">{{ t("config.inherit") }}</span>
                    </button>
                    <button
                      v-for="mode in modeOptions"
                      :key="mode"
                      type="button"
                      class="strategy-option"
                      :class="{ selected: ruleFor(module).default_mode === mode }"
                      @click="ruleFor(module).default_mode = mode"
                    >
                      <span class="opt-title">{{ modeLabels[mode] }}</span>
                    </button>
                  </div>
                </section>

                <section class="body-section">
                  <span class="section-label">{{ t("config.paths") }}</span>
                  <div
                    v-for="(mode, path) in ruleFor(module).paths"
                    :key="path"
                    class="rule-path-row module-path-row"
                  >
                    <span class="rule-path-label">{{ path }}</span>
                    <Md3SelectField
                      compact
                      :label="String(path)"
                      :model-value="mode"
                      :options="modeSelectOptions"
                      @update:model-value="
                        ruleFor(module).paths[path] = $event as MountMode
                      "
                    />
                    <md-icon-button
                      :aria-label="t('common.close')"
                      @click="delete ruleFor(module).paths[path]"
                    >
                      <md-icon
                        ><svg viewBox="0 0 24 24"><path :d="ICONS.delete" /></svg
                      ></md-icon>
                    </md-icon-button>
                  </div>
                  <div class="rule-path-row module-path-row new-path-row">
                    <input
                      v-model="newPaths[module.id]"
                      class="md3-input-native"
                      :placeholder="t('config.pathPlaceholder')"
                    />
                    <Md3SelectField
                      compact
                      :label="t('config.defaultMode')"
                      :model-value="newModes[module.id] ?? 'overlay'"
                      :options="modeSelectOptions"
                      @update:model-value="newModes[module.id] = $event as MountMode"
                    />
                    <md-filled-tonal-icon-button
                      :title="t('config.addPathRule')"
                      :aria-label="t('config.addPathRule')"
                      @click="addPath(module)"
                    >
                      <md-icon
                        ><svg viewBox="0 0 24 24"><path :d="ICONS.add" /></svg
                      ></md-icon>
                    </md-filled-tonal-icon-button>
                  </div>
                </section>

                <div class="module-actions">
                  <md-filled-tonal-icon-button
                    class="module-icon-action save-module-action"
                    :title="t('modules.save')"
                    :aria-label="t('modules.save')"
                    @click="saveRules(module)"
                  >
                    <md-icon
                      ><svg viewBox="0 0 24 24"><path :d="ICONS.save" /></svg
                    ></md-icon>
                  </md-filled-tonal-icon-button>
                </div>
              </div>
            </div>
          </div>
        </Transition>
      </article>
    </section>

    <div v-if="moduleStore.loading" class="loading-container">
      <md-circular-progress indeterminate />
    </div>
    <div v-else-if="filtered.length === 0" class="empty-state">
      <svg class="empty-icon" viewBox="0 0 24 24"><path :d="ICONS.modules" /></svg>
      <strong>{{ t("modules.empty") }}</strong>
      <span class="empty-state-hint">{{ t("modules.desc") }}</span>
    </div>
  </div>
</template>

<style scoped>
.header-section {
  flex-direction: row;
  align-items: center;
  gap: 8px;
}

.search-bar {
  min-width: 0;
  flex: 1;
}

.refresh-modules-action {
  flex: 0 0 44px;
  width: 44px;
  height: 44px;
  --md-filled-tonal-icon-button-container-shape: var(--radius-md);
  --md-filled-tonal-icon-button-container-color: var(
    --md-sys-color-surface-container-high
  );
  --md-filled-tonal-icon-button-icon-color: var(--md-sys-color-on-surface-variant);
}

.modules-page > .error-banner {
  flex-direction: row;
  align-items: center;
  gap: 12px;
  margin-bottom: 0;
  padding: 10px 12px;
  border: 0;
  border-radius: var(--radius-lg);
}

.error-content {
  gap: 2px;
}
</style>
