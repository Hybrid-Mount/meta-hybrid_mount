<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { moduleStore } from "../../../lib/stores/moduleStore";
import { sysStore } from "../../../lib/stores/sysStore";
import { uiStore } from "../../../lib/stores/uiStore";
import type { Module, ModuleRule, MountMode } from "../../../lib/types";

const { t } = useI18n();

const modeOptions: MountMode[] = ["overlay", "magic", "ignore"];
const modeLabels = computed(() => [
  t("config.modeOverlay"),
  t("config.modeMagic"),
  t("config.modeIgnore"),
]);

const query = ref("");
const filter = ref<"all" | MountMode>("all");
const filtered = computed(() =>
  moduleStore.modules.filter((module) => {
    if (filter.value !== "all" && module.mode !== filter.value) return false;
    const needle = query.value.trim().toLowerCase();
    if (!needle) return true;
    return (
      module.name.toLowerCase().includes(needle) ||
      module.id.toLowerCase().includes(needle) ||
      module.description.toLowerCase().includes(needle)
    );
  }),
);

const expanded = ref<Record<string, boolean>>({});
const editing = ref<Record<string, ModuleRule>>({});

function ruleFor(module: Module): ModuleRule {
  if (!editing.value[module.id]) {
    const fallback: MountMode | null =
      module.rules.default_mode === "magic" ||
      module.rules.default_mode === "ignore" ||
      module.rules.default_mode === "overlay"
        ? module.rules.default_mode
        : null;
    editing.value[module.id] = {
      default_mode: fallback,
      paths: { ...module.rules.paths } as Record<string, MountMode>,
    };
  }
  return editing.value[module.id];
}

async function saveRules(module: Module): Promise<void> {
  const ok = await moduleStore.saveModuleRules(module.id, ruleFor(module));
  uiStore.showToast(ok ? t("modules.saveSuccess") : t("modules.saveFailed"));
}

async function clearErrors(): Promise<void> {
  const removed = await sysStore.clearMountErrors();
  uiStore.showToast(t("modules.clearedCount", { count: removed }));
}

onMounted(async () => {
  await moduleStore.loadModules();
});
</script>

<template>
  <div class="page">
    <div class="md3-card">
      <div class="md3-field">
        <input
          v-model="query"
          class="md3-input"
          :placeholder="t('modules.searchPlaceholder')"
        />
        <select v-model="filter" class="md3-select">
          <option value="all">
            {{ t("modules.filterAll") }}
          </option>
          <option v-for="(option, index) in modeOptions" :key="option" :value="option">
            {{ modeLabels[index] }}
          </option>
        </select>
      </div>
    </div>

    <div v-for="module in filtered" :key="module.id" class="md3-card">
      <h4>{{ module.name || module.id }}</h4>
      <p>
        {{ module.id }} · v{{ module.version }} · {{ module.author }} ·
        {{ modeLabels[modeOptions.indexOf(module.mode)] }}
      </p>
      <p v-if="module.mount_error">
        ⚠ {{ t("modules.mountError") }}: {{ module.mount_error }}
      </p>
      <p v-if="module.suggest_ignore">⚠ {{ t("modules.suggestIgnore") }}</p>
      <p>{{ module.description || t("modules.noDescriptionLabel") }}</p>

      <div class="md3-actions">
        <button class="md3-button" @click="expanded[module.id] = !expanded[module.id]">
          {{
            expanded[module.id] ? t("modules.collapseRules") : t("modules.expandRules")
          }}
        </button>
      </div>

      <template v-if="expanded[module.id]">
        <div class="md3-field">
          <label>{{ t("config.moduleDefault") }}</label>
          <select
            :value="ruleFor(module).default_mode ?? ''"
            class="md3-select"
            @change="
              ruleFor(module).default_mode = ($event.target as HTMLSelectElement).value
                ? (($event.target as HTMLSelectElement).value as MountMode)
                : null
            "
          >
            <option value="">
              {{ t("config.inherit") }}
            </option>
            <option v-for="(option, index) in modeOptions" :key="option" :value="option">
              {{ modeLabels[index] }}
            </option>
          </select>
        </div>
        <div v-for="(mode, path) in ruleFor(module).paths" :key="path" class="md3-field">
          <label>{{ path }}</label>
          <select
            :value="mode"
            class="md3-select"
            @change="
              ruleFor(module).paths[path] = ($event.target as HTMLSelectElement)
                .value as MountMode
            "
          >
            <option v-for="(option, index) in modeOptions" :key="option" :value="option">
              {{ modeLabels[index] }}
            </option>
          </select>
        </div>
        <div class="md3-actions">
          <button class="md3-button md3-button-primary" @click="saveRules(module)">
            {{ t("modules.save") }}
          </button>
        </div>
      </template>
    </div>

    <div v-if="filtered.length === 0" class="md3-card">
      <p>{{ t("modules.empty") }}</p>
    </div>

    <div class="md3-card">
      <div class="md3-actions">
        <button class="md3-button" @click="clearErrors">
          {{ t("modules.clearErrors") }}
        </button>
        <button class="md3-button" @click="moduleStore.loadModules()">
          {{ t("modules.reload") }}
        </button>
      </div>
    </div>
  </div>
</template>
