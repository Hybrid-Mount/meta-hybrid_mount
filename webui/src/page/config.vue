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
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import {
  getCurrentLangIndex,
  switchLocale,
  getSupportedLocales,
} from "../locales";
import {
  MiuixCard,
  MiuixSmallTitle,
  MiuixText,
  MiuixSwitch,
  MiuixButton,
  MiuixRadioButtonPreference,
  MiuixDropdownPreference,
  MiuixInput,
  MiuixBasicComponent,
  MiuixIcon,
  MiuixDialog,
  showSnackbar,
} from "miuix-vue";
import {
  FolderFill,
  MoveFile,
  Settings,
  ChevronForward,
} from "miuix-vue/icons";
import { Motion, AnimatePresence } from "motion-v";
import { API } from "../lib/api";
import type { AppConfig, OverlayMode } from "../lib/types";
const { t } = useI18n();

const expandSpring = { type: "spring" as const, stiffness: 400, damping: 40 };

const text1 = ref("");
const text2 = ref("");
const display_list = ref<string[]>([]);
const lang_code = ref<string[]>([]);
const lang_dropdown_index = ref(0);

const expand_overlay = ref(false);
const overlay_workmode = ref<OverlayMode>("tmpfs");
const daemon = ref(false);
const umount_disabled = ref(false);
const reset_req = ref(false);

let currentConfig: AppConfig | null = null;

onMounted(async () => {
  try {
    // Load supported locales
    const supported_locales = await getSupportedLocales();
    const lang_index = await getCurrentLangIndex();
    lang_dropdown_index.value = lang_index;
    display_list.value = supported_locales.map(
      (supported_locales) => supported_locales.display,
    );
    lang_code.value = supported_locales.map(
      (supported_locales) => supported_locales.code,
    );

    // Load config from API
    currentConfig = await API.loadConfig();
    text1.value = currentConfig.moduledir;
    text2.value = currentConfig.mountsource;
    overlay_workmode.value = currentConfig.overlay_mode;
    umount_disabled.value = currentConfig.disable_umount;
    daemon.value = currentConfig.daemon_startup_mode === "persistent";
  } catch (e) {
    console.error("Failed to load config:", e);
  }
});

function handleChange(value: number) {
  switchLocale(lang_code.value[value]);
  window.location.reload();
}

async function save_config() {
  if (!currentConfig) return;
  try {
    const newConfig: AppConfig = {
      ...currentConfig,
      moduledir: text1.value,
      mountsource: text2.value,
      overlay_mode: overlay_workmode.value,
      disable_umount: umount_disabled.value,
      daemon_startup_mode: daemon.value ? "persistent" : "on-demand",
    };
    await API.saveConfig(newConfig);
    currentConfig = newConfig;
    showSnackbar({ message: t("common.saved") || "Config saved" });
  } catch (e) {
    console.error("Failed to save config:", e);
    showSnackbar({ message: "Failed to save config" });
  }
}

function reset_config() {
  reset_req.value = false;
  API.resetConfig()
    .then(() => {
      showSnackbar({ message: t("config.resetSuccess") || "Config reset" });
      // Reload config after reset
      return API.loadConfig();
    })
    .then((config) => {
      if (config) {
        currentConfig = config;
        text1.value = config.moduledir;
        text2.value = config.mountsource;
        overlay_workmode.value = config.overlay_mode;
        umount_disabled.value = config.disable_umount;
        daemon.value = config.daemon_startup_mode === "persistent";
      }
    })
    .catch((e) => {
      console.error("Failed to reset config:", e);
    });
}
</script>

<template>
  <div class="page">
    <MiuixSmallTitle :text="t('config.webui')" />
    <MiuixCard class="ex-card">
      <MiuixDropdownPreference
        :title="t('common.language')"
        :summary="lang_code[lang_dropdown_index]"
        v-model="lang_dropdown_index"
        :items="display_list"
      />
      <div style="padding: 12px; display: flex">
        <MiuixButton
          class="ex-glow"
          type="primary"
          @click="handleChange(lang_dropdown_index)"
        >
          {{ t("config.save") }}
        </MiuixButton>
      </div>
    </MiuixCard>

    <MiuixSmallTitle :text="t('tabs.config')" />
    <MiuixCard class="ex-card">
      <MiuixBasicComponent
        :title="t('config.moduleDir')"
        :summary="t('config.moduleDirDesc')"
      >
        <template #start>
          <MiuixIcon :icon="FolderFill" />
        </template>
      </MiuixBasicComponent>
      <div style="padding: 0 16px 16px">
        <MiuixInput
          v-model="text1"
          :label="t('config.moduleDir')"
          single-line
        />
      </div>

      <MiuixBasicComponent
        :title="t('config.mountSource')"
        :summary="t('config.mountSourceDesc')"
      >
        <template #start>
          <MiuixIcon :icon="MoveFile" />
        </template>
      </MiuixBasicComponent>
      <div style="padding: 0 16px 16px">
        <MiuixInput
          v-model="text2"
          :label="t('config.mountSource')"
          single-line
        />
      </div>
      <MiuixBasicComponent
        :title="t('config.overlayMode')"
        :summary="t('config.overlayModeDesc')"
        :clickable="true"
        @click="expand_overlay = !expand_overlay"
      >
        <template #start>
          <MiuixIcon :icon="Settings" />
        </template>
        <template #end>
          <MiuixIcon :icon="ChevronForward" :size="16" />
        </template>
      </MiuixBasicComponent>
      <AnimatePresence :initial="false">
        <Motion
          v-if="expand_overlay"
          class="ex-expand"
          :initial="{ height: 0, opacity: 0 }"
          :animate="{ height: 'auto', opacity: 1 }"
          :exit="{ height: 0, opacity: 0 }"
          :transition="expandSpring"
        >
          <MiuixRadioButtonPreference
            :model-value="overlay_workmode === 'tmpfs'"
            :title="t('config.mode_tmpfs')"
            :summary="t('config.mode_tmpfsDesc')"
            @select="overlay_workmode = 'tmpfs'"
          />
          <MiuixRadioButtonPreference
            :model-value="overlay_workmode === 'ext4'"
            :title="t('config.mode_ext4')"
            :summary="t('config.mode_ext4Desc')"
            @select="overlay_workmode = 'ext4'"
          />
        </Motion>
      </AnimatePresence>

      <MiuixBasicComponent>
        <template #start>
          <MiuixText>{{ t("config.disableUmount") }}</MiuixText>
        </template>
        <template #end>
          <MiuixSwitch v-model="umount_disabled" label="Enabled" />
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent>
        <template #start>
          <MiuixText>{{ t("config.daemonStartupMode") }}</MiuixText>
        </template>
        <template #end>
          <MiuixSwitch v-model="daemon" label="Enabled" />
        </template>
      </MiuixBasicComponent>
      <div style="padding: 12px; display: flex">
        <MiuixButton class="ex-glow" @click="reset_req = true">
          {{ t("config.resetConfig") }}
        </MiuixButton>
        <MiuixButton class="ex-glow" type="primary" @click="save_config">
          {{ t("config.save") }}
        </MiuixButton>
      </div>
    </MiuixCard>

    <MiuixDialog
      v-model="reset_req"
      :title="t('config.resetConfigTitle')"
      :summary="t('config.resetConfigConfirm')"
      @close="reset_req = false"
    >
      <template #default="{ close }">
        <div class="ex-dialog-actions">
          <MiuixButton class="ex-grow" @click="close">
            {{ t("common.cancel") }}
          </MiuixButton>
          <MiuixButton class="ex-grow" type="primary" @click="reset_config()">
            {{ t("config.resetConfig") }}
          </MiuixButton>
        </div>
      </template>
    </MiuixDialog>
  </div>
</template>

<style scoped>
.ex-card {
  margin: 0 12px 12px;
}
.ex-dialog-actions {
  display: flex;
  gap: 12px;
}
.ex-glow {
  flex: 1;
}
.ex-grow {
  flex: 1;
}
</style>
