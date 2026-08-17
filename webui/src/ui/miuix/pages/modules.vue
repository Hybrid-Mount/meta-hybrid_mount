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
  MiuixButton,
  MiuixDropdownPreference,
} from "miuix-vue";
import { moduleStore } from "../../../lib/stores/moduleStore";
import { uiStore } from "../../../lib/stores/uiStore";
import { sysStore } from "../../../lib/stores/sysStore";
import type { Module, ModuleRule, MountMode } from "../../../lib/types";

const { t } = useI18n();

const searchQuery = ref("");
const filterIndex = ref(0);
const filterModes: (MountMode | "all")[] = ["all", "overlay", "magic", "ignore"];
const modeOptions: MountMode[] = ["overlay", "magic", "ignore"];
const modeLabels = computed(() => [
  t("config.modeOverlay"),
  t("config.modeMagic"),
  t("config.modeIgnore"),
]);
const filterLabels = computed(() => [t("modules.filterAll"), ...modeLabels.value]);

const filteredModules = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  const filter = filterModes[filterIndex.value] ?? "all";
  return moduleStore.modules.filter((module) => {
    if (filter !== "all" && module.mode !== filter) return false;
    if (!query) return true;
    return (
      module.name.toLowerCase().includes(query) ||
      module.description.toLowerCase().includes(query) ||
      module.id.toLowerCase().includes(query)
    );
  });
});

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

onMounted(async () => {
  await moduleStore.loadModules();
});
</script>

<template>
  <div class="page">
    <MiuixSearchBar v-model="searchQuery" :placeholder="t('modules.searchPlaceholder')" />
    <MiuixDropdownPreference
      v-model="filterIndex"
      :title="t('modules.filterLabel')"
      :items="filterLabels"
    />

    <MiuixProgressIndicator v-if="moduleStore.loading" indeterminate />

    <template v-for="module in filteredModules" :key="module.id">
      <MiuixCard class="card">
        <MiuixBasicComponent
          :title="module.name || module.id"
          :summary="`${module.id} · v${module.version} · ${module.author}`"
        >
          <template #end>
            <MiuixText :color="module.mode === 'ignore' ? 'error' : 'success'">
              {{ modeLabel(module.mode) }}
            </MiuixText>
          </template>
        </MiuixBasicComponent>

        <MiuixBasicComponent
          v-if="module.mount_error"
          :title="t('modules.mountError')"
          :summary="module.mount_error"
        />
        <MiuixBasicComponent
          v-if="module.suggest_ignore"
          :title="t('modules.suggestIgnore')"
        />

        <MiuixBasicComponent
          :title="t('modules.descriptionLabel')"
          :summary="module.description || t('modules.noDescriptionLabel')"
        />

        <div class="actions">
          <MiuixButton @click="expanded[module.id] = !expanded[module.id]">
            {{
              expanded[module.id] ? t("modules.collapseRules") : t("modules.expandRules")
            }}
          </MiuixButton>
        </div>

        <template v-if="expanded[module.id]">
          <div class="rule-row">
            <span>{{ t("config.moduleDefault") }}</span>
            <select
              :value="ruleFor(module).default_mode ?? ''"
              class="select"
              @change="
                ruleFor(module).default_mode = ($event.target as HTMLSelectElement).value
                  ? (($event.target as HTMLSelectElement).value as MountMode)
                  : null
              "
            >
              <option value="">
                {{ t("config.inherit") }}
              </option>
              <option
                v-for="(option, index) in modeOptions"
                :key="option"
                :value="option"
              >
                {{ modeLabels[index] }}
              </option>
            </select>
          </div>
          <div v-for="(mode, path) in ruleFor(module).paths" :key="path" class="rule-row">
            <span class="path">{{ path }}</span>
            <select
              :value="mode"
              class="select"
              @change="
                ruleFor(module).paths[path] = ($event.target as HTMLSelectElement)
                  .value as MountMode
              "
            >
              <option
                v-for="(option, index) in modeOptions"
                :key="option"
                :value="option"
              >
                {{ modeLabels[index] }}
              </option>
            </select>
          </div>
          <MiuixButton
            :disabled="savingModule === module.id"
            @click="saveModuleRules(module)"
          >
            {{ t("modules.save") }}
          </MiuixButton>
        </template>
      </MiuixCard>
    </template>

    <MiuixBasicComponent
      v-if="!moduleStore.loading && filteredModules.length === 0"
      :title="t('modules.empty')"
    />

    <MiuixCard class="card">
      <div class="actions">
        <MiuixButton @click="clearErrors">
          {{ t("modules.clearErrors") }}
        </MiuixButton>
        <MiuixButton @click="moduleStore.loadModules()">
          {{ t("modules.reload") }}
        </MiuixButton>
      </div>
    </MiuixCard>
  </div>
</template>

<style scoped>
.actions {
  display: flex;
  gap: 8px;
  margin: 8px 0;
}

.rule-row {
  display: flex;
  gap: 8px;
  align-items: center;
  margin: 6px 0;
}

.rule-row .path {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.select {
  min-width: 0;
  padding: 6px 8px;
  border-radius: 8px;
  border: 1px solid var(--m-color-outline-variant, rgba(0, 0, 0, 0.2));
  background: var(--m-color-surface, #fff);
}
</style>
