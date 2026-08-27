<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { moduleStore } from "../../../lib/stores/moduleStore";
import { sysStore } from "../../../lib/stores/sysStore";
import { uiStore } from "../../../lib/stores/uiStore";
import { matchesModuleFilter, type ModuleFilter } from "../../../lib/moduleFilter";
import type { Module, ModuleRule, MountMode } from "../../../lib/types";
import Md3BottomActions from "../components/Md3BottomActions.vue";
import Md3SelectField, { type SelectOption } from "../components/Md3SelectField.vue";
import { ICONS } from "../icons";

const { t } = useI18n();
const modeOptions: MountMode[] = ["overlay", "magic", "ignore"];
const modeLabels = computed<Record<MountMode, string>>(() => ({
  overlay: t("config.modeOverlay"),
  magic: t("config.modeMagic"),
  ignore: t("config.modeIgnore"),
}));
const modeSelectOptions = computed<SelectOption[]>(() =>
  modeOptions.map((mode) => ({ value: mode, label: modeLabels.value[mode] })),
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

onMounted(() => moduleStore.ensureModulesLoaded());
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
          'has-error': Boolean(module.mount_error),
        }"
      >
        <button
          type="button"
          class="module-header"
          :aria-expanded="Boolean(expanded[module.id])"
          @click="expanded[module.id] = !expanded[module.id]"
        >
          <span class="mode-indicator" :class="`mode-${module.mode}`" />
          <span class="module-info">
            <span class="module-name">{{ module.name || module.id }}</span>
            <span v-if="expanded[module.id]" class="module-id">{{ module.id }}</span>
            <span class="module-meta">
              <span class="version-badge">v{{ module.version || "-" }}</span>
              <span>{{ module.author || t("modules.unknownLabel") }}</span>
            </span>
          </span>
          <span class="mode-pill">{{ modeLabels[module.mode] }}</span>
        </button>

        <div v-if="expanded[module.id]" class="module-body-wrapper">
          <div class="module-body-inner">
            <div class="module-body-content">
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

    <Md3BottomActions>
      <md-filled-tonal-icon-button
        :title="t('modules.reload')"
        :aria-label="t('modules.reload')"
        @click="moduleStore.loadModules()"
      >
        <md-icon
          ><svg viewBox="0 0 24 24"><path :d="ICONS.refresh" /></svg
        ></md-icon>
      </md-filled-tonal-icon-button>
    </Md3BottomActions>
  </div>
</template>
