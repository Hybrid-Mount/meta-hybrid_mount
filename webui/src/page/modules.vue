<!--

    Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.

-->
<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  showSnackbar,
  MiuixSearchBar,
  MiuixCard,
  MiuixBasicComponent,
  MiuixSmallTitle,
  MiuixRadioButtonPreference,
  MiuixButton,
  MiuixText,
  MiuixIcon,
} from "miuix-vue";
import { Close } from "miuix-vue/icons";
import { Motion, AnimatePresence } from "motion-v";
const { t } = useI18n();
import { API } from "../lib/api";
import type { Module, MountMode } from "../lib/types";

// Extended module type with UI state
type ModuleWithUI = Module & {
  Bottomopen?: boolean;
};

const expandSpring = { type: "spring" as const, stiffness: 400, damping: 40 };

const searchQuery = ref("");
const searchexpanded = ref(false);
const modules_list = ref<ModuleWithUI[]>([]);
const kasumi_supported = ref(false);

const filterModules = computed(() => {
  if (searchQuery.value.trim() === "") {
    return modules_list.value;
  }
  const query = searchQuery.value.toLowerCase();
  return modules_list.value.filter(
    (module) =>
      module.name.toLowerCase().includes(query) ||
      module.description.toLowerCase().includes(query) ||
      module.id.toLowerCase().includes(query),
  );
});

function run_hybird_api_command_modules_set_mode(
  module_id: string,
  mode: MountMode,
) {
  return API.saveModuleRules(module_id, {
    default_mode: mode,
    paths: {},
  }).finally(() => {
    showSnackbar({
      message: t("modules.saveSuccess"),
    });
  });
}

onMounted(async () => {
  try {
    // Check kasumi availability from init
    const initPayload = await API.init();
    if (initPayload.kasumi_status) {
      kasumi_supported.value = initPayload.kasumi_status.available;
    }

    // Load modules
    const modules = await API.scanModules();

    // Add Bottomopen property for UI expansion state
    modules_list.value = modules.map((m) => ({ ...m, Bottomopen: false }));

    console.info(modules_list.value);
  } catch (e) {
    console.error("Failed to load modules:", e);
  }
});
</script>

<template>
  <div class="page">
    <div class="icon-search">
      <MiuixSearchBar
        v-model="searchQuery"
        v-model:expanded="searchexpanded"
        :label="t('modules.searchPlaceholder')"
      ></MiuixSearchBar>
    </div>
    <div
      v-if="modules_list.length === 0 || filterModules.length === 0"
      align="center"
    >
      <MiuixText type="subtitle" class="ex-card">
        {{ t("modules.emptyState") }}
      </MiuixText>
    </div>
    <div v-else v-for="module in filterModules">
      <MiuixCard class="ex-card">
        <MiuixBasicComponent
          :title="module.name"
          :summary="module.id + ' ' + module.version"
          :clickable="true"
          @click="module.Bottomopen = !module.Bottomopen"
        >
          <template #end>
            <MiuixText
              v-if="module.mount_error"
              type="body2"
              color="var(--m-color-error)"
            >
              {{ t("modules.mountError") }}
            </MiuixText>
            <MiuixText
              v-else-if="module.mode === 'ignore'"
              type="body2"
              color="var(--m-color-on-surface-variant-actions)"
            >
              {{ t("modules.modes.unmounted") }}
            </MiuixText>
            <MiuixText
              v-else
              type="body2"
              color="var(--m-color-on-surface-variant-actions)"
            >
              {{
                module.is_mounted
                  ? t("modules.modes." + module.mode)
                  : t("modules.modes.unmounted")
              }}
            </MiuixText>
          </template>
        </MiuixBasicComponent>
        <AnimatePresence :initial="false">
          <Motion
            v-if="module.Bottomopen"
            class="ex-expand"
            :initial="{ height: 0, opacity: 0 }"
            :animate="{ height: 'auto', opacity: 1 }"
            :exit="{ height: 0, opacity: 0 }"
            :transition="expandSpring"
          >
            <MiuixBasicComponent :summary="module.description" />
            <MiuixCard
              v-if="module.mount_error"
              style="--m-card-color: var(--m-color-error)"
            >
              <MiuixBasicComponent
                :title="t('modules.mountError') + ': ' + module.mount_error"
                titleColor="var(--m-color-on-error)"
                :summary="t('modules.suggestIgnoreHint')"
                summaryColor="var(--m-color-on-error)"
              >
                <template #start>
                  <MiuixIcon :icon="Close" color="var(--m-color-on-error)" />
                </template>
              </MiuixBasicComponent>
            </MiuixCard>
            <MiuixSmallTitle>{{ t("modules.defaultMode") }}</MiuixSmallTitle>
            <div class="ex-basic-row">
              <MiuixRadioButtonPreference
                class="ex-row-child"
                :model-value="module.mode === 'overlay'"
                :title="t('modules.modes.short.overlay')"
                :summary="t('modules.defaultTag')"
                @click="module.mode = 'overlay'"
              />
              <MiuixRadioButtonPreference
                class="ex-row-child"
                :model-value="module.mode === 'magic'"
                :title="t('modules.modes.short.magic')"
                :summary="t('modules.compatTag')"
                @click="module.mode = 'magic'"
              />

              <MiuixRadioButtonPreference
                v-if="kasumi_supported"
                class="ex-row-child"
                :model-value="module.mode === 'kasumi'"
                :title="t('modules.modes.short.kasumi')"
                :summary="t('modules.nativeTag')"
                @click="module.mode = 'kasumi'"
              />
              <MiuixRadioButtonPreference
                class="ex-row-child"
                :model-value="module.mode === 'ignore'"
                :title="t('modules.modes.short.ignore')"
                :summary="t('modules.disableTag')"
                @click="module.mode = 'ignore'"
              />
              <MiuixRadioButtonPreference
                v-if="!kasumi_supported"
                disabled="true"
                class="ex-row-child"
              />
            </div>
            <div style="display: flex">
              <MiuixButton
                class="ex-glow"
                type="primary"
                @click="
                  run_hybird_api_command_modules_set_mode(
                    module.id,
                    module.mode,
                  )
                "
              >
                {{ t("modules.save") }}
              </MiuixButton>
            </div>
          </Motion>
        </AnimatePresence>
      </MiuixCard>
    </div>
  </div>
</template>

<style scoped>
.icon-search {
  padding: 0 0 6px;
}
.ex-card {
  margin: 0 12px 12px;
}
.ex-basic-row {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}
.ex-row-child {
  flex: 1;
}
.ex-glow {
  flex: 1;
}
</style>
