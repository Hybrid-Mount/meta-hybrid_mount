<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { uiStore } from "../../../lib/stores/uiStore";
import { configStore } from "../../../lib/stores/configStore";
import type { DefaultMountMode, OverlayMode, UiStyle } from "../../../lib/types";
import Md3BottomActions from "../components/Md3BottomActions.vue";
import Md3SelectField, { type SelectOption } from "../components/Md3SelectField.vue";
import { ICONS } from "../icons";

const { t } = useI18n();
const resetOpen = ref(false);
const modeOptions: DefaultMountMode[] = ["overlay", "magic"];
const modeLabels = computed<Record<DefaultMountMode, string>>(() => ({
  overlay: t("config.modeOverlay"),
  magic: t("config.modeMagic"),
}));
const languageOptions = computed<SelectOption[]>(() =>
  uiStore.availableLanguages.map((language) => ({
    value: language.code,
    label: language.display,
  })),
);
const styleOptions: SelectOption[] = [
  { value: "md3", label: "Material Design 3" },
  { value: "miuix", label: "Miuix" },
];
const overlayOptions = computed<SelectOption[]>(() => {
  const options: SelectOption[] = [];
  if (configStore.config.tmpfs_xattr_supported) {
    options.push({ value: "tmpfs", label: t("config.overlayTmpfs") });
  }
  options.push({ value: "ext4", label: t("config.overlayExt4") });
  return options;
});
function eventValue(event: Event): string {
  return String((event.target as HTMLInputElement & { value: string }).value ?? "");
}

function eventSelected(event: Event): boolean {
  return Boolean((event.target as HTMLElement & { selected?: boolean }).selected);
}

function updateConfig(patch: Partial<typeof configStore.config>): void {
  configStore.setConfig({ ...configStore.config, ...patch });
}

function updateUiStyle(value: string): void {
  uiStore.setUiStyle(value as UiStyle);
  uiStore.showToast(t("common.styleChanged"));
}

async function save(): Promise<void> {
  const ok = await configStore.saveConfig();
  uiStore.showToast(ok ? t("config.saveSuccess") : t("config.saveFailed"));
}

async function reset(): Promise<void> {
  resetOpen.value = false;
  const ok = await configStore.resetConfig();
  uiStore.showToast(ok ? t("config.resetSuccess") : t("config.resetFailed"));
}

onMounted(() => configStore.ensureConfigLoaded());
</script>

<template>
  <div class="config-container">
    <section class="config-group">
      <div class="config-card">
        <div class="card-header">
          <span class="card-icon">
            <md-icon
              ><svg viewBox="0 0 24 24"><path :d="ICONS.settings" /></svg
            ></md-icon>
          </span>
          <span class="card-text">
            <span class="card-title">WebUI</span>
            <span class="card-desc"
              >{{ t("common.language") }} · {{ t("common.uiStyle") }}</span
            >
          </span>
        </div>
        <div class="field-grid">
          <Md3SelectField
            :label="t('common.language')"
            :model-value="uiStore.lang"
            :options="languageOptions"
            @update:model-value="uiStore.setLang"
          />
          <Md3SelectField
            :label="t('common.uiStyle')"
            :model-value="uiStore.uiStyle"
            :options="styleOptions"
            @update:model-value="updateUiStyle"
          />
        </div>
      </div>
    </section>

    <section class="config-group">
      <div class="config-card">
        <div class="card-header">
          <span class="card-icon">
            <md-icon
              ><svg viewBox="0 0 24 24"><path :d="ICONS.settings" /></svg
            ></md-icon>
          </span>
          <span class="card-text">
            <span class="card-title">{{ t("config.title") }}</span>
            <span class="card-desc">{{ t("config.applyHint") }}</span>
          </span>
        </div>
        <div class="field-grid configuration-fields">
          <md-outlined-text-field
            data-testid="module-directory-field"
            class="full-width-field full-span"
            :label="t('config.moduledir')"
            :supporting-text="t('config.moduledirDesc')"
            :value="configStore.config.moduledir"
            @input="updateConfig({ moduledir: eventValue($event) })"
          />
          <md-outlined-text-field
            data-testid="mount-source-field"
            class="full-width-field full-span"
            :label="t('config.mountSource')"
            :supporting-text="t('config.mountSourceDesc')"
            :value="configStore.config.mountsource"
            @input="updateConfig({ mountsource: eventValue($event) })"
          />
          <Md3SelectField
            :label="t('config.overlayMode')"
            :model-value="configStore.config.overlay_mode"
            :options="overlayOptions"
            @update:model-value="updateConfig({ overlay_mode: $event as OverlayMode })"
          />
          <div class="setting-line">
            <span class="setting-copy">
              <strong>{{ t("config.disableUmount") }}</strong>
              <span>{{ t("config.disableUmountDesc") }}</span>
            </span>
            <md-switch
              :selected="configStore.config.disable_umount"
              @change="updateConfig({ disable_umount: eventSelected($event) })"
            />
          </div>
        </div>
      </div>

      <div class="config-card">
        <div class="card-header">
          <span class="card-icon">
            <md-icon
              ><svg viewBox="0 0 24 24"><path :d="ICONS.storage" /></svg
            ></md-icon>
          </span>
          <span class="card-text">
            <span class="card-title">{{ t("config.defaultMode") }}</span>
            <span class="card-desc">{{ t("config.defaultModeDesc") }}</span>
          </span>
        </div>
        <div class="mode-selector">
          <button
            v-for="mode in modeOptions"
            :key="mode"
            type="button"
            class="mode-item"
            :class="{ selected: configStore.config.default_mode === mode }"
            @click="updateConfig({ default_mode: mode })"
          >
            <span class="mode-info">
              <span class="mode-title">{{ modeLabels[mode] }}</span>
            </span>
            <span class="mode-check">
              <md-icon
                ><svg viewBox="0 0 24 24"><path :d="ICONS.check" /></svg
              ></md-icon>
            </span>
          </button>
        </div>
      </div>
    </section>

    <Md3BottomActions>
      <md-filled-tonal-icon-button
        class="destructive-action"
        :title="t('common.reset')"
        :aria-label="t('common.reset')"
        @click="resetOpen = true"
      >
        <md-icon
          ><svg viewBox="0 0 24 24"><path :d="ICONS.reset" /></svg
        ></md-icon>
      </md-filled-tonal-icon-button>
      <md-filled-tonal-icon-button
        :title="t('config.save')"
        :aria-label="t('config.save')"
        @click="save"
      >
        <md-icon
          ><svg viewBox="0 0 24 24"><path :d="ICONS.save" /></svg
        ></md-icon>
      </md-filled-tonal-icon-button>
    </Md3BottomActions>

    <md-dialog :open="resetOpen" @closed="resetOpen = false">
      <div slot="headline">{{ t("common.reset") }}</div>
      <div slot="content">{{ t("config.resetConfirm") }}</div>
      <div slot="actions">
        <md-text-button @click="resetOpen = false">{{
          t("common.cancel")
        }}</md-text-button>
        <md-text-button @click="reset">{{ t("common.reset") }}</md-text-button>
      </div>
    </md-dialog>
  </div>
</template>
