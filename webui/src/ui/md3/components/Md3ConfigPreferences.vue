<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { uiStore } from "../../../lib/stores/uiStore";
import type { UiStyle } from "../../../lib/types";
import Md3SelectField, { type SelectOption } from "./Md3SelectField.vue";
import { ICONS } from "../icons";

defineProps<{ wear?: boolean }>();
const { t } = useI18n();
const languageOptions = computed(() =>
  uiStore.availableLanguages.map((language) => ({
    value: language.code,
    label: language.display,
  })),
);
const styleOptions: SelectOption[] = [
  { value: "md3", label: "Material Design 3" },
  { value: "miuix", label: "Miuix" },
];
function updateUiStyle(value: string): void {
  uiStore.setUiStyle(value as UiStyle);
  uiStore.showToast(t("common.styleChanged"));
}
</script>

<template>
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
          <span v-if="!wear" class="card-desc"
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
          v-if="!wear"
          :label="t('common.uiStyle')"
          :model-value="uiStore.uiStyle"
          :options="styleOptions"
          @update:model-value="updateUiStyle"
        />
      </div>
    </div>
  </section>
</template>
