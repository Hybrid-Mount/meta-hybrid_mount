<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  MiuixCard,
  MiuixSmallTitle,
  MiuixSwitch,
  MiuixButton,
  MiuixBasicComponent,
  MiuixDialog,
  MiuixIcon,
  MiuixIconButton,
  IconCheck,
} from "miuix-vue";
import { Reset } from "miuix-vue/icons";
import { getSupportedLocales } from "../../../locales";
import { uiStore } from "../../../lib/stores/uiStore";
import { configStore } from "../../../lib/stores/configStore";
import type { MountMode } from "../../../lib/types";
import MiuixSelectField, {
  type MiuixSelectOption,
} from "../components/MiuixSelectField.vue";

const { t } = useI18n();

const styleOptions: MiuixSelectOption[] = [
  { value: "miuix", label: "MiuiX" },
  { value: "md3", label: "Material Design 3" },
];
const modeOptions: MountMode[] = ["overlay", "magic", "ignore"];
const modeLabels = computed(() => [
  t("config.modeOverlay"),
  t("config.modeMagic"),
  t("config.modeIgnore"),
]);
const modeSelectOptions = computed<MiuixSelectOption[]>(() =>
  modeOptions.map((mode, index) => ({
    value: mode,
    label: modeLabels.value[index],
  })),
);
const overlayOptions = computed<MiuixSelectOption[]>(() => [
  { value: "tmpfs", label: t("config.overlayTmpfs") },
  { value: "ext4", label: t("config.overlayExt4") },
]);

const languageOptions = ref<MiuixSelectOption[]>([]);
const resetRequested = ref(false);

const uiStyleValue = computed({
  get: () => uiStore.uiStyle,
  set: (value: string) => {
    uiStore.setUiStyle(value as "miuix" | "md3");
    uiStore.showToast(t("common.styleChanged"));
  },
});

const languageValue = computed({
  get: () => uiStore.lang,
  set: (value: string) => uiStore.setLang(value),
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
const overlayModeValue = computed({
  get: () => configStore.config.overlay_mode,
  set: (value: string) =>
    configStore.setConfig({
      ...configStore.config,
      overlay_mode: value as "tmpfs" | "ext4",
    }),
});
const defaultModeValue = computed({
  get: () => configStore.config.default_mode,
  set: (value: string) =>
    configStore.setConfig({
      ...configStore.config,
      default_mode: value as MountMode,
    }),
});

async function save(): Promise<void> {
  const ok = await configStore.saveConfig();
  uiStore.showToast(ok ? t("config.saveSuccess") : t("config.saveFailed"));
}

async function reset(): Promise<void> {
  resetRequested.value = false;
  const ok = await configStore.resetConfig();
  uiStore.showToast(ok ? t("config.resetSuccess") : t("config.resetFailed"));
}

onMounted(async () => {
  await configStore.ensureConfigLoaded();
  const locales = await getSupportedLocales();
  languageOptions.value = locales.map((locale) => ({
    value: locale.code,
    label: locale.display,
  }));
});
</script>

<template>
  <div class="page">
    <MiuixCard class="card">
      <MiuixSelectField
        v-model="languageValue"
        :label="t('common.language')"
        :options="languageOptions"
      />
      <MiuixSelectField
        v-model="uiStyleValue"
        :label="t('common.uiStyle')"
        :options="styleOptions"
      />
      <MiuixBasicComponent :title="t('config.monetTheme')">
        <template #end>
          <MiuixSwitch v-model="monetTheme" />
        </template>
      </MiuixBasicComponent>
    </MiuixCard>

    <MiuixSmallTitle :text="t('config.title')" />
    <MiuixCard class="card">
      <MiuixBasicComponent
        class="text-preference"
        :title="t('config.moduledir')"
        :summary="t('config.moduledirDesc')"
      >
        <template #end>
          <input
            v-model="moduledir"
            class="preference-input"
            :aria-label="t('config.moduledir')"
            autocomplete="off"
            spellcheck="false"
          />
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        class="text-preference"
        :title="t('config.mountSource')"
        :summary="t('config.mountSourceDesc')"
      >
        <template #end>
          <input
            v-model="mountSource"
            class="preference-input"
            :aria-label="t('config.mountSource')"
            autocomplete="off"
            spellcheck="false"
          />
        </template>
      </MiuixBasicComponent>
      <MiuixSelectField
        v-model="overlayModeValue"
        :label="t('config.overlayMode')"
        :summary="t('config.overlayModeDesc')"
        :options="overlayOptions"
      />
      <MiuixSelectField
        v-model="defaultModeValue"
        :label="t('config.defaultMode')"
        :summary="t('config.defaultModeDesc')"
        :options="modeSelectOptions"
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

    <MiuixCard class="card action-card">
      <div class="config-action-bar">
        <span class="action-hint">{{ t("config.applyHint") }}</span>
        <div class="icon-actions" role="group" :aria-label="t('config.title')">
          <MiuixIconButton
            class="save-action"
            :title="t('config.save')"
            :aria-label="t('config.save')"
            @click="save"
          >
            <IconCheck class="check-icon" />
          </MiuixIconButton>
          <MiuixIconButton
            class="reset-action"
            :title="t('common.reset')"
            :aria-label="t('common.reset')"
            @click="resetRequested = true"
          >
            <MiuixIcon :icon="Reset" :size="22" />
          </MiuixIconButton>
        </div>
      </div>
    </MiuixCard>

    <MiuixDialog
      v-model="resetRequested"
      :title="t('common.reset')"
      :summary="t('config.resetConfirm')"
      @close="resetRequested = false"
    >
      <template #default="{ close }">
        <div class="reset-dialog-actions">
          <MiuixButton class="grow" @click="close">
            {{ t("common.cancel") }}
          </MiuixButton>
          <MiuixButton class="grow" type="primary" @click="reset">
            {{ t("common.reset") }}
          </MiuixButton>
        </div>
      </template>
    </MiuixDialog>
  </div>
</template>

<style scoped>
.preference-input {
  width: min(280px, 46vw);
  min-width: 0;
  min-height: 40px;
  box-sizing: border-box;
  border: 1px solid transparent;
  border-radius: 13px;
  padding: 0 12px;
  color: var(--m-color-on-surface, #1d1b20);
  background: var(--m-color-surface-container-high, rgba(0, 0, 0, 0.06));
  font: inherit;
  text-align: end;
}

.preference-input:hover {
  border-color: var(--m-color-outline-variant, rgba(0, 0, 0, 0.18));
}

.preference-input:focus-visible {
  outline: 2px solid var(--m-color-primary, #6750a4);
  outline-offset: 2px;
}

.action-card {
  overflow: visible;
}

.config-action-bar {
  min-height: 64px;
  padding: 8px 12px 8px 16px;
  display: flex;
  align-items: center;
  gap: 16px;
}

.action-hint {
  min-width: 0;
  flex: 1;
  color: var(--m-color-on-surface-variant-summary, rgba(0, 0, 0, 0.6));
  font-size: 13px;
  line-height: 18px;
}

.icon-actions,
.reset-dialog-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.save-action,
.reset-action {
  --m-icon-button-min-width: 44px;
  --m-icon-button-min-height: 44px;
  --m-icon-button-radius: 15px;
}

.save-action {
  --m-icon-button-bg: var(--m-color-primary, #6750a4);
  color: var(--m-color-on-primary, #fff);
}

.reset-action {
  --m-icon-button-bg: var(--m-color-surface-container-high, rgba(0, 0, 0, 0.06));
}

.check-icon {
  width: 22px;
  height: 22px;
  color: currentColor;
}

.reset-dialog-actions .grow {
  flex: 1;
}

@media (max-width: 520px) {
  .preference-input {
    width: min(180px, 43vw);
  }
}

@media (max-width: 420px) {
  .text-preference :deep(.m-basic-component__row) {
    align-items: stretch;
    flex-direction: column;
    gap: 10px;
  }

  .text-preference :deep(.m-basic-component__end),
  .preference-input {
    width: 100%;
  }

  .preference-input {
    text-align: start;
  }
}
</style>
