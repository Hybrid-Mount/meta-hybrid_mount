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
const modeOptions: MountMode[] = ["overlay", "magic", "ignore"];
const modeLabels = computed(() => [
  t("config.modeOverlay"),
  t("config.modeMagic"),
  t("config.modeIgnore"),
]);
const filterOptions = computed<MiuixSelectOption[]>(() => [
  { value: "active", label: t("modules.filterActive") },
  { value: "all", label: t("modules.filterAll") },
  ...modeOptions.map((mode, index) => ({
    value: mode,
    label: modeLabels.value[index],
  })),
]);
const ruleModeOptions = computed<MiuixSelectOption[]>(() =>
  modeOptions.map((mode, index) => ({
    value: mode,
    label: modeLabels.value[index],
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
  return modeLabels.value[modeOptions.indexOf(mode)];
}

function toggleModule(moduleId: string): void {
  expanded.value[moduleId] = !expanded.value[moduleId];
}

onMounted(() => moduleStore.ensureModulesLoaded());
</script>

<template>
  <div class="page">
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
    </div>

    <MiuixProgressIndicator v-if="moduleStore.loading" indeterminate />

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
          :summary="`${module.id} · v${module.version} · ${module.author}`"
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
              <MiuixText :color="module.mode === 'ignore' ? 'error' : 'success'">
                {{ modeLabel(module.mode) }}
              </MiuixText>
              <MiuixIcon
                :icon="expanded[module.id] ? ExpandLess : ExpandMore"
                :size="20"
                aria-hidden="true"
              />
            </span>
          </template>
        </MiuixBasicComponent>

        <Transition name="module-details">
          <div v-if="expanded[module.id]" class="module-details">
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
                @update:model-value="ruleFor(module).paths[path] = $event as MountMode"
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
        </Transition>
      </MiuixCard>
    </template>

    <MiuixBasicComponent
      v-if="!moduleStore.loading && filteredModules.length === 0"
      :title="t('modules.empty')"
    />

    <div class="actions">
      <MiuixIconButton
        class="module-icon-action reload-modules-button"
        :title="t('modules.reload')"
        :aria-label="t('modules.reload')"
        @click="moduleStore.loadModules()"
      >
        <MiuixIcon :icon="Refresh" :size="22" />
      </MiuixIconButton>
    </div>
  </div>
</template>

<style scoped>
.module-header {
  cursor: pointer;
  border-radius: 18px;
}

.module-header :deep(.m-basic-component__row) {
  width: 100%;
  min-width: 0;
}

.modules-toolbar {
  width: auto;
  margin-inline: 12px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(160px, 190px);
  gap: 12px;
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
  padding: 10px 16px;
  display: flex;
  align-items: center;
  gap: 12px;
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
  color: var(--m-color-on-surface-variant, rgba(0, 0, 0, 0.6));
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

.module-details {
  padding: 4px 16px 16px;
  border-top: 1px solid var(--m-color-outline-variant, rgba(0, 0, 0, 0.08));
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin: 12px 0;
}

.rule-row {
  display: flex;
  gap: 12px;
  align-items: center;
  min-height: 48px;
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
  margin-top: 20px;
  padding-top: 4px;
}

.module-icon-action {
  --m-icon-button-min-width: 44px;
  --m-icon-button-min-height: 44px;
  --m-icon-button-radius: 15px;
}

.save-module-button,
.reload-modules-button {
  --m-icon-button-bg: var(--m-color-primary, #6750a4);
  color: var(--m-color-on-primary, #fff);
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
  transition:
    opacity 160ms ease,
    transform 160ms ease;
}

.module-details-enter-from,
.module-details-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

@media (max-width: 560px) {
  .modules-toolbar {
    grid-template-columns: minmax(0, 1fr) minmax(120px, 38vw);
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
