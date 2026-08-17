<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  MiuixCard,
  MiuixSmallTitle,
  MiuixSwitch,
  MiuixButton,
  MiuixDropdownPreference,
  MiuixInput,
  MiuixBasicComponent,
} from "miuix-vue";
import { getSupportedLocales } from "../../../locales";
import { uiStore } from "../../../lib/stores/uiStore";
import { configStore } from "../../../lib/stores/configStore";
import { moduleStore } from "../../../lib/stores/moduleStore";
import type { ModuleRule, MountMode } from "../../../lib/types";

const { t } = useI18n();

const styleOptions = ["MiuiX", "Material Design 3"];
const styleCodes: ("miuix" | "md3")[] = ["miuix", "md3"];
const modeOptions: MountMode[] = ["overlay", "magic", "ignore"];
const modeLabels = computed(() => [
  t("config.modeOverlay"),
  t("config.modeMagic"),
  t("config.modeIgnore"),
]);
const overlayOptions = [t("config.overlayTmpfs"), t("config.overlayExt4")];
const overlayCodes: ("tmpfs" | "ext4")[] = ["tmpfs", "ext4"];

const languageDisplay = ref<string[]>([]);
const languageCodes = ref<string[]>([]);
const currentLangIndex = ref(0);

const uiStyleIndex = computed({
  get: () => styleCodes.indexOf(uiStore.uiStyle),
  set: (value: number) => {
    uiStore.setUiStyle(styleCodes[value]);
    uiStore.showToast(t("common.styleChanged"));
  },
});

const languageIndex = computed({
  get: () => currentLangIndex.value,
  set: async (value: number) => {
    currentLangIndex.value = value;
    await uiStore.setLang(languageCodes.value[value]);
  },
});

const monetTheme = computed({
  get: () => uiStore.monetEnabled,
  set: (value: boolean) => uiStore.setMonetEnabled(value),
});

const moduledir = computed({
  get: () => configStore.config.moduledir,
  set: (value: string) =>
    configStore.setConfig({ ...configStore.config, moduledir: value }),
});
const mountSource = computed({
  get: () => configStore.config.mountsource,
  set: (value: string) =>
    configStore.setConfig({ ...configStore.config, mountsource: value }),
});
const disableUmount = computed({
  get: () => configStore.config.disable_umount,
  set: (value: boolean) =>
    configStore.setConfig({ ...configStore.config, disable_umount: value }),
});
const overlayModeIndex = computed({
  get: () => overlayCodes.indexOf(configStore.config.overlay_mode),
  set: (value: number) =>
    configStore.setConfig({
      ...configStore.config,
      overlay_mode: overlayCodes[value],
    }),
});
const defaultModeIndex = computed({
  get: () => modeOptions.indexOf(configStore.config.default_mode),
  set: (value: number) =>
    configStore.setConfig({
      ...configStore.config,
      default_mode: modeOptions[value],
    }),
});

const rules = ref<Record<string, ModuleRule>>({});
watch(
  () => configStore.config.rules,
  (next) => {
    rules.value = JSON.parse(JSON.stringify(next)) as Record<string, ModuleRule>;
  },
  { immediate: true, deep: true },
);

const selectedModule = ref("");
const newRulePath = ref("");
const newRuleMode = ref<MountMode>("overlay");

function ensureRule(moduleId: string): ModuleRule {
  if (!rules.value[moduleId]) {
    rules.value[moduleId] = { default_mode: null, paths: {} };
  }
  return rules.value[moduleId];
}

function addModuleRule(): void {
  if (!selectedModule.value) return;
  ensureRule(selectedModule.value);
}

function addPathRule(moduleId: string): void {
  const path = newRulePath.value.trim();
  if (!path) return;
  ensureRule(moduleId).paths[path] = newRuleMode.value;
  newRulePath.value = "";
}

function removePathRule(moduleId: string, path: string): void {
  const rule = rules.value[moduleId];
  if (!rule) return;
  delete rule.paths[path];
}

function removeModuleRule(moduleId: string): void {
  delete rules.value[moduleId];
}

async function save(): Promise<void> {
  configStore.setConfig({ ...configStore.config, rules: rules.value });
  const ok = await configStore.saveConfig();
  uiStore.showToast(ok ? t("config.saveSuccess") : t("config.saveFailed"));
}

async function reset(): Promise<void> {
  const ok = await configStore.resetConfig();
  rules.value = {};
  uiStore.showToast(ok ? t("config.resetSuccess") : t("config.resetFailed"));
}

onMounted(async () => {
  await Promise.all([configStore.loadConfig(), moduleStore.ensureModulesLoaded()]);
  const locales = await getSupportedLocales();
  languageDisplay.value = locales.map((locale) => locale.display);
  languageCodes.value = locales.map((locale) => locale.code);
  currentLangIndex.value = languageCodes.value.indexOf(uiStore.lang);
});
</script>

<template>
  <div class="page">
    <MiuixCard class="card">
      <MiuixDropdownPreference
        v-model="languageIndex"
        :title="t('common.language')"
        :items="languageDisplay"
      />
      <MiuixDropdownPreference
        v-model="uiStyleIndex"
        :title="t('config.uiStyle')"
        :items="styleOptions"
      />
      <MiuixBasicComponent :title="t('config.monetTheme')">
        <template #end>
          <MiuixSwitch v-model="monetTheme" />
        </template>
      </MiuixBasicComponent>
    </MiuixCard>

    <MiuixSmallTitle :text="t('config.title')" />
    <MiuixCard class="card">
      <MiuixInput v-model="moduledir" :label="t('config.moduledir')" single-line />
      <MiuixInput v-model="mountSource" :label="t('config.mountSource')" single-line />
      <MiuixDropdownPreference
        v-model="overlayModeIndex"
        :title="t('config.overlayMode')"
        :summary="t('config.overlayModeDesc')"
        :items="overlayOptions"
      />
      <MiuixDropdownPreference
        v-model="defaultModeIndex"
        :title="t('config.defaultMode')"
        :summary="t('config.defaultModeDesc')"
        :items="modeLabels"
      />
      <MiuixBasicComponent
        :title="t('config.disableUmount')"
        :summary="t('config.disableUmountDesc')"
      >
        <template #end>
          <MiuixSwitch v-model="disableUmount" />
        </template>
      </MiuixBasicComponent>
    </MiuixCard>

    <MiuixSmallTitle :text="t('config.rulesTitle')" />
    <MiuixCard class="card">
      <div class="rule-add">
        <select v-model="selectedModule" class="select">
          <option value="" disabled>
            {{ t("config.moduleDefault") }}
          </option>
          <option
            v-for="module in moduleStore.modules"
            :key="module.id"
            :value="module.id"
          >
            {{ module.name || module.id }}
          </option>
        </select>
        <MiuixButton @click="addModuleRule">
          {{ t("config.addPathRule") }}
        </MiuixButton>
      </div>

      <div v-for="moduleId in Object.keys(rules)" :key="moduleId" class="rule-block">
        <div class="rule-header">
          <strong>{{ moduleId }}</strong>
          <MiuixButton @click="removeModuleRule(moduleId)">
            {{ t("common.reset") }}
          </MiuixButton>
        </div>
        <label class="rule-field">
          {{ t("config.moduleDefault") }}
          <select
            :value="rules[moduleId].default_mode ?? ''"
            class="select"
            @change="
              rules[moduleId].default_mode = ($event.target as HTMLSelectElement).value
                ? (($event.target as HTMLSelectElement).value as MountMode)
                : null
            "
          >
            <option value="">{{ t("config.inherit") }}</option>
            <option v-for="(mode, index) in modeOptions" :key="mode" :value="mode">
              {{ modeLabels[index] }}
            </option>
          </select>
        </label>
        <div v-for="(mode, path) in rules[moduleId].paths" :key="path" class="rule-field">
          <span class="rule-path">{{ path }}</span>
          <select
            :value="mode"
            class="select"
            @change="
              rules[moduleId].paths[path] = ($event.target as HTMLSelectElement)
                .value as MountMode
            "
          >
            <option v-for="(option, index) in modeOptions" :key="option" :value="option">
              {{ modeLabels[index] }}
            </option>
          </select>
          <MiuixButton @click="removePathRule(moduleId, path)">
            {{ t("common.close") }}
          </MiuixButton>
        </div>
        <div class="rule-add">
          <MiuixInput
            v-model="newRulePath"
            :label="t('config.pathPlaceholder')"
            single-line
          />
          <select v-model="newRuleMode" class="select">
            <option v-for="(option, index) in modeOptions" :key="option" :value="option">
              {{ modeLabels[index] }}
            </option>
          </select>
          <MiuixButton @click="addPathRule(moduleId)">
            {{ t("common.save") }}
          </MiuixButton>
        </div>
      </div>
    </MiuixCard>

    <MiuixCard class="card">
      <div class="actions">
        <MiuixButton @click="save">
          {{ t("config.save") }}
        </MiuixButton>
        <MiuixButton @click="reset">
          {{ t("common.reset") }}
        </MiuixButton>
      </div>
      <MiuixBasicComponent :summary="t('config.applyHint')" />
    </MiuixCard>
  </div>
</template>

<style scoped>
.rule-add {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
  margin: 8px 0;
}

.rule-block {
  border-top: 1px solid var(--m-color-outline-variant, rgba(0, 0, 0, 0.08));
  padding: 8px 0;
}

.rule-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.rule-field {
  display: flex;
  gap: 8px;
  align-items: center;
  margin: 6px 0;
}

.rule-path {
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

.actions {
  display: flex;
  gap: 12px;
  margin-bottom: 8px;
}
</style>
