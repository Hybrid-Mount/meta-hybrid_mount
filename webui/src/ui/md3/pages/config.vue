<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { getSupportedLocales } from "../../../locales";
import { uiStore } from "../../../lib/stores/uiStore";
import { configStore } from "../../../lib/stores/configStore";
import { moduleStore } from "../../../lib/stores/moduleStore";
import type { ModuleRule, MountMode } from "../../../lib/types";

const { t } = useI18n();

const modeOptions: MountMode[] = ["overlay", "magic", "ignore"];
const modeLabels = computed(() => [
  t("config.modeOverlay"),
  t("config.modeMagic"),
  t("config.modeIgnore"),
]);

const languages = ref<{ code: string; display: string }[]>([]);
const language = computed({
  get: () => uiStore.lang,
  set: async (value: string) => uiStore.setLang(value),
});
const uiStyle = computed({
  get: () => uiStore.uiStyle,
  set: (value: "miuix" | "md3") => {
    uiStore.setUiStyle(value);
    uiStore.showToast(t("common.styleChanged"));
  },
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
const overlayMode = computed({
  get: () => configStore.config.overlay_mode,
  set: (value: "tmpfs" | "ext4") =>
    configStore.setConfig({ ...configStore.config, overlay_mode: value }),
});
const defaultMode = computed({
  get: () => configStore.config.default_mode,
  set: (value: MountMode) =>
    configStore.setConfig({ ...configStore.config, default_mode: value }),
});
const disableUmount = computed({
  get: () => configStore.config.disable_umount,
  set: (value: boolean) =>
    configStore.setConfig({ ...configStore.config, disable_umount: value }),
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
const newPath = ref("");
const newPathMode = ref<MountMode>("overlay");

function addModule(): void {
  if (selectedModule.value && !rules.value[selectedModule.value]) {
    rules.value[selectedModule.value] = { default_mode: null, paths: {} };
  }
}

function addPath(moduleId: string): void {
  const path = newPath.value.trim();
  if (!path) return;
  rules.value[moduleId] = rules.value[moduleId] ?? { default_mode: null, paths: {} };
  rules.value[moduleId].paths[path] = newPathMode.value;
  newPath.value = "";
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
  languages.value = await getSupportedLocales();
});
</script>

<template>
  <div class="page">
    <div class="md3-card">
      <div class="md3-field">
        <label>{{ t("common.language") }}</label>
        <select v-model="language" class="md3-select">
          <option v-for="locale in languages" :key="locale.code" :value="locale.code">
            {{ locale.display }}
          </option>
        </select>
      </div>
      <div class="md3-field">
        <label>{{ t("config.uiStyle") }}</label>
        <select v-model="uiStyle" class="md3-select">
          <option value="miuix">MiuiX</option>
          <option value="md3">Material Design 3</option>
        </select>
      </div>
    </div>

    <div class="md3-card">
      <div class="md3-field">
        <label>{{ t("config.moduledir") }}</label>
        <input v-model="moduledir" class="md3-input" />
      </div>
      <div class="md3-field">
        <label>{{ t("config.mountSource") }}</label>
        <input v-model="mountSource" class="md3-input" />
      </div>
      <div class="md3-field">
        <label>{{ t("config.overlayMode") }}</label>
        <select v-model="overlayMode" class="md3-select">
          <option value="tmpfs">
            {{ t("config.overlayTmpfs") }}
          </option>
          <option value="ext4">
            {{ t("config.overlayExt4") }}
          </option>
        </select>
      </div>
      <div class="md3-field">
        <label>{{ t("config.defaultMode") }}</label>
        <select v-model="defaultMode" class="md3-select">
          <option v-for="(option, index) in modeOptions" :key="option" :value="option">
            {{ modeLabels[index] }}
          </option>
        </select>
      </div>
      <label class="md3-field">
        <span
          ><input v-model="disableUmount" type="checkbox" />
          {{ t("config.disableUmount") }}</span
        >
      </label>
    </div>

    <div class="md3-card">
      <h4>{{ t("config.rulesTitle") }}</h4>
      <div class="md3-field">
        <select v-model="selectedModule" class="md3-select">
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
        <div class="md3-actions">
          <button class="md3-button" @click="addModule">
            {{ t("config.addPathRule") }}
          </button>
        </div>
      </div>

      <div v-for="moduleId in Object.keys(rules)" :key="moduleId" class="md3-field">
        <label>{{ moduleId }}</label>
        <select
          :value="rules[moduleId].default_mode ?? ''"
          class="md3-select"
          @change="
            rules[moduleId].default_mode = ($event.target as HTMLSelectElement).value
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
        <div v-for="(mode, path) in rules[moduleId].paths" :key="path" class="md3-field">
          <label>{{ path }}</label>
          <div class="md3-actions">
            <select
              :value="mode"
              class="md3-select"
              @change="
                rules[moduleId].paths[path] = ($event.target as HTMLSelectElement)
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
            <button class="md3-button" @click="delete rules[moduleId].paths[path]">
              {{ t("common.close") }}
            </button>
          </div>
        </div>
        <div class="md3-actions">
          <input
            v-model="newPath"
            class="md3-input"
            :placeholder="t('config.pathPlaceholder')"
          />
          <select v-model="newPathMode" class="md3-select">
            <option v-for="(option, index) in modeOptions" :key="option" :value="option">
              {{ modeLabels[index] }}
            </option>
          </select>
          <button class="md3-button" @click="addPath(moduleId)">
            {{ t("common.save") }}
          </button>
        </div>
      </div>
    </div>

    <div class="md3-card">
      <div class="md3-actions">
        <button class="md3-button md3-button-primary" @click="save">
          {{ t("config.save") }}
        </button>
        <button class="md3-button" @click="reset">
          {{ t("common.reset") }}
        </button>
      </div>
      <p>{{ t("config.applyHint") }}</p>
    </div>
  </div>
</template>
