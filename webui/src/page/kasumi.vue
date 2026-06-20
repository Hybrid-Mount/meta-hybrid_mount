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
<!-- TODO: Add kasumi settings,wait anantx complete the next api version -->
<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import {
  MiuixCard,
  MiuixSwitch,
  MiuixText,
  MiuixBasicComponent,
  MiuixIcon,
} from "miuix-vue";
import { Info, Close, ChevronForward } from "miuix-vue/icons";
import { Motion, AnimatePresence } from "motion-v";
import { API } from "../lib/api";
import type { KasumiStatus } from "../lib/types";
import { showSnackbar } from "miuix-vue";

const kasumi = ref<KasumiStatus | null>(null);
const show_lkm_settings = ref(false);
const expandSpring = { type: "spring" as const, stiffness: 400, damping: 40 };

async function toggleKasumiEnabled(enabled: boolean) {
  if (!kasumi.value) return;
  try {
    await API.setKasumiEnabled(enabled);
    // Reload status after toggle
    kasumi.value = await API.getKasumiStatus();
    showSnackbar({
      message: enabled
        ? t("config.kasumiEnabledSuccess") || "Kasumi enabled"
        : t("config.kasumiDisabledSuccess") || "Kasumi disabled",
    });
  } catch (e) {
    console.error("Failed to toggle kasumi:", e);
  }
}

onMounted(async () => {
  try {
    await API.wakeDaemon();
    kasumi.value = await API.getKasumiStatus();
    console.info("Kasumi status:", kasumi.value);
  } catch (e) {
    console.error("Failed to load kasumi status:", e);
  }
});
</script>

<template>
  <div class="page">
    <div v-if="!kasumi?.available || !kasumi?.lkm?.loaded">
      <MiuixCard
        class="ex-card ex-card--pad"
        show-indication
        style="--m-card-color: var(--m-color-error)"
      >
        <MiuixBasicComponent
          :title="t('kasumi.title')"
          titleColor="var(--m-color-on-error)"
        >
          <template #start>
            <MiuixIcon :icon="Close" color="var(--m-color-on-error)" />
          </template>
          <template #end>
            <MiuixText style="color: var(--m-color-on-error)">
              <b>
                {{
                  kasumi?.available
                    ? t("kasumi.statusUnavailable")
                    : t("kasumi.statusDisabled")
                }}
              </b>
            </MiuixText>
          </template>
        </MiuixBasicComponent>
      </MiuixCard>
    </div>
    <div v-else-if="kasumi?.available && kasumi?.lkm?.loaded">
      <MiuixCard
        class="ex-card ex-card--pad"
        show-indication
        style="--m-card-color: var(--m-color-primary-variant)"
      >
        <MiuixBasicComponent
          :title="t('kasumi.title')"
          titleColor="var(--m-color-on-primary-variant)"
          :summary="kasumi.mirror_path"
          summaryColor="var(--m-color-on-primary-variant)"
        >
          <template #start>
            <MiuixIcon :icon="Info" color="var(--m-color-on-primary-variant)" />
          </template>
          <template #end>
            <MiuixText
              :size="18"
              weight="medium"
              style="color: var(--m-color-on-primary-variant)"
            >
              {{ t("kasumi.statusWorking") }}
            </MiuixText>
          </template>
        </MiuixBasicComponent>
      </MiuixCard>
      <div class="ex-card-row">
        <MiuixCard class="ex-card--pad ex-grow">
          <MiuixBasicComponent title="API">
            <template #end>
              <MiuixText :size="18" weight="medium">
                <b>{{ kasumi.protocol_version }}</b>
              </MiuixText>
            </template>
          </MiuixBasicComponent>
        </MiuixCard>
        <MiuixCard class="ex-card--pad ex-grow">
          <MiuixBasicComponent :title="t('kasumi.rulesBadge')">
            <template #end>
              <MiuixText :size="18" weight="medium">
                <b>{{ kasumi.rule_count }}</b>
              </MiuixText>
            </template>
          </MiuixBasicComponent>
        </MiuixCard>
      </div>
    </div>

    <MiuixCard class="ex-card">
      <MiuixBasicComponent>
        <template #start>
          <MiuixText>{{ t("config.kasumiMasterSwitch") }}</MiuixText>
        </template>
        <template #end>
          <MiuixSwitch
            :model-value="kasumi?.config?.enabled"
            label="Enabled"
            :disabled="kasumi?.available && !kasumi?.config?.enabled"
            @update:model-value="toggleKasumiEnabled"
          />
        </template>
      </MiuixBasicComponent>
    </MiuixCard>
    <AnimatePresence :initial="false">
      <Motion
        v-if="kasumi?.config?.enabled"
        class="ex-expand"
        :initial="{ height: 0, opacity: 0 }"
        :animate="{ height: 'auto', opacity: 1 }"
        :exit="{ height: 0, opacity: 0 }"
        :transition="expandSpring"
      >
        <MiuixCard class="ex-card">
          <MiuixBasicComponent
            :title="t('kasumi.lkmTitle')"
            :clickable="true"
            @click="show_lkm_settings = !show_lkm_settings"
          >
            <template #end>
              <MiuixText :size="18" weight="medium">
                {{ t("kasumi.autoloadOn") }}
              </MiuixText>
              <span class="ex-gap8" />
              <MiuixIcon :icon="ChevronForward" :size="16" />
            </template>
          </MiuixBasicComponent>
          <AnimatePresence :initial="false">
            <Motion
              v-if="show_lkm_settings"
              class="ex-expand"
              :initial="{ height: 0, opacity: 0 }"
              :animate="{ height: 'auto', opacity: 1 }"
              :exit="{ height: 0, opacity: 0 }"
              :transition="expandSpring"
            >
              <MiuixBasicComponent :title="t('kasumi.currentKmi')">
                <template #end>
                  <MiuixText>{{ kasumi?.lkm?.current_kmi }}</MiuixText>
                </template>
              </MiuixBasicComponent>
            </Motion>
          </AnimatePresence>

          <MiuixBasicComponent :title="t('kasumi.runtimeTitle')">
            <template #end>
              <MiuixIcon :icon="ChevronForward" :size="16" />
            </template>
          </MiuixBasicComponent>

          <MiuixBasicComponent :title="t('kasumi.identityTitle')">
            <template #end>
              <MiuixText :size="14">
                {{ t("kasumi.unameModeScoped") }}
              </MiuixText>
              <span class="ex-gap8" />
              <MiuixIcon :icon="ChevronForward" :size="16" />
            </template>
          </MiuixBasicComponent>

          <MiuixBasicComponent :title="t('kasumi.userHideTitle')">
            <template #end>
              <MiuixIcon :icon="ChevronForward" :size="16" />
            </template>
          </MiuixBasicComponent>

          <MiuixBasicComponent :title="t('kasumi.mapsTitle')">
            <template #end>
              <MiuixText :size="14">
                {{ kasumi?.user_hide_rule_count }}
              </MiuixText>
              <span class="ex-gap8" />
              <MiuixIcon :icon="ChevronForward" :size="16" />
            </template>
          </MiuixBasicComponent>

          <MiuixBasicComponent :title="t('kasumi.featuresTitle')">
            <template #end>
              <MiuixIcon :icon="ChevronForward" :size="16" />
            </template>
          </MiuixBasicComponent>
        </MiuixCard>
      </Motion>
    </AnimatePresence>
  </div>
</template>

<style scoped>
.ex-card {
  margin: 0 12px 12px;

  &--pad .m-card {
    padding: 16px;
  }
}
.ex-card-row {
  display: flex;
  gap: 12px;
  margin: 0 12px 12px;
}
.ex-expand {
  overflow: hidden;
}
.ex-basic-row {
  display: flex;
  gap: 12px;
}
.ex-grow {
  flex: 1;
}
.ex-gap8 {
  width: 8px;
}
</style>
