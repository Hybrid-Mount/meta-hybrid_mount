<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { uiStore } from "../../../lib/stores/uiStore";
import { configStore } from "../../../lib/stores/configStore";
import { sysStore } from "../../../lib/stores/sysStore";
import type { DefaultMountMode, OverlayMode } from "../../../lib/types";
import Md3ActionButton from "../components/Md3ActionButton.vue";
import Md3ConfigPreferences from "../components/Md3ConfigPreferences.vue";
import Md3BottomActions from "../components/Md3BottomActions.vue";
import Md3SelectField, { type SelectOption } from "../components/Md3SelectField.vue";
import { ICONS } from "../icons";

defineProps<{ wear?: boolean }>();
const { t } = useI18n();
const resetOpen = ref(false);
const modeOptions = computed(() => sysStore.defaultMountModes);
const modeLabels = computed<Record<DefaultMountMode, string>>(() => ({
  overlay: t("config.modeOverlay"),
  magic: t("config.modeMagic"),
  vfs: t("config.modeVfs"),
}));
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

async function save(): Promise<void> {
  const ok = await configStore.saveConfig();
  uiStore.showToast(ok ? t("config.saveSuccess") : t("config.saveFailed"));
}

async function reset(): Promise<void> {
  resetOpen.value = false;
  const ok = await configStore.resetConfig();
  uiStore.showToast(ok ? t("config.resetSuccess") : t("config.resetFailed"));
}

onMounted(() =>
  Promise.all([sysStore.ensureStatusLoaded(), configStore.ensureConfigLoaded()]),
);
</script>

<template>
  <div class="config-container">
    <Md3ConfigPreferences v-if="!wear" />

    <section class="config-group" :inert="wear && configStore.loading">
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
              :aria-label="wear ? t('config.disableUmount') : undefined"
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
            :aria-pressed="wear ? configStore.config.default_mode === mode : undefined"
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
      <Md3ActionButton
        :wear="wear"
        class="destructive-action"
        :label="t('common.reset')"
        :icon="ICONS.reset"
        :disabled="wear && (configStore.loading || configStore.saving)"
        @click="resetOpen = true"
      />
      <Md3ActionButton
        :wear="wear"
        :label="t('config.save')"
        :icon="ICONS.save"
        :disabled="wear && (!configStore.hasLoaded || configStore.saving)"
        @click="save"
      />
    </Md3BottomActions>

    <Md3ConfigPreferences v-if="wear" wear />

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
