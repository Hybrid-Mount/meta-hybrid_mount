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
const { t } = useI18n();
import {
  MiuixCard,
  MiuixSmallTitle,
  MiuixBasicComponent,
  MiuixIcon,
  MiuixText,
} from "miuix-vue";
import { Info } from "miuix-vue/icons";
import { API } from "../lib/api";
import { ENABLE_KASUMI } from "../lib/constants_gen";

const storage_path = ref("/dev/homo/114514");
const storage_mode = ref("HomoFS");
const actived_count = ref(0);
const kernel_version = ref("5.14.114-homo");
const selinux = ref("Enforcing");
const mount_source = ref("HOMO");
const overlay_count = ref(1);
const magic_count = ref(1);
const kasumi_count = ref(4);
const activeMounts = ref<string[]>([]);

onMounted(async () => {
  try {
    // Wake daemon first
    await API.wakeDaemon();

    // Get initial payload with all data
    const initPayload = await API.init();

    // Set system info
    if (initPayload.system_info) {
      kernel_version.value = initPayload.system_info.kernel;
      selinux.value = initPayload.system_info.selinux;
      activeMounts.value = initPayload.system_info.activeMounts || [];
    }

    // Set storage info
    if (initPayload.status) {
      storage_mode.value = initPayload.status.storage_mode || "unknown";
      storage_path.value =
        initPayload.status.mount_point || "/data/adb/hybrid-mount/mnt";

      // Get mode stats
      const modeStats = initPayload.status.mode_stats;
      if (modeStats) {
        overlay_count.value = modeStats.overlayfs || 0;
        magic_count.value = modeStats.magicmount || 0;
        kasumi_count.value = ENABLE_KASUMI ? modeStats.kasumi || 0 : 0;
      }

      // Calculate active count
      const overlay = initPayload.status.overlay_modules?.length || 0;
      const magic = initPayload.status.magic_modules?.length || 0;
      const kasumi = initPayload.status.kasumi_modules?.length || 0;
      actived_count.value = overlay + magic + kasumi;
    }

    // Set config info
    if (initPayload.config) {
      mount_source.value = initPayload.config.mountsource;
    }
  } catch (e) {
    console.error("Failed to load status data:", e);
  }
});
</script>

<template>
  <div class="page">
    <MiuixCard
      class="ex-card ex-card--pad"
      show-indication
      style="--m-card-color: var(--m-color-primary-variant)"
    >
      <MiuixBasicComponent
        :title="t('status.storageTitle')"
        titleColor="var(--m-color-on-primary-variant)"
        :summary="storage_path"
        summaryColor="var(--m-color-on-primary-variant)"
      >
        <template #start>
          <MiuixIcon color="var(--m-color-on-primary-variant)" :icon="Info" />
        </template>
        <template #end>
          <MiuixText style="color: var(--m-color-on-primary-variant)">
            {{ storage_mode }}
          </MiuixText>
        </template>
      </MiuixBasicComponent>
    </MiuixCard>
    <div class="ex-card-row">
      <MiuixCard class="ex-card--pad ex-grow">
        <MiuixBasicComponent :title="t('status.moduleActive')">
          <template #end>
            <MiuixText :size="18" weight="medium">
              <b>{{ actived_count }}</b>
            </MiuixText>
          </template>
        </MiuixBasicComponent>
      </MiuixCard>
      <MiuixCard class="ex-card--pad ex-grow">
        <MiuixBasicComponent :title="t('status.mountBase')">
          <template #end>
            <MiuixText :size="18" weight="medium">
              <b>{{ mount_source }}</b>
            </MiuixText>
          </template>
        </MiuixBasicComponent>
      </MiuixCard>
    </div>
    <MiuixSmallTitle :text="t('status.modeStats')" />
    <MiuixCard class="ex-card">
      <div class="ex-basic-row ex-grow">
        <MiuixBasicComponent title="Overlay">
          <template #end>
            <MiuixText>{{ overlay_count }}</MiuixText>
          </template>
        </MiuixBasicComponent>
        <MiuixBasicComponent title="Magic">
          <template #end>
            <MiuixText>{{ magic_count }}</MiuixText>
          </template>
        </MiuixBasicComponent>
        <MiuixBasicComponent title="Kasumi">
          <template #end>
            <MiuixText>{{ kasumi_count }}</MiuixText>
          </template>
        </MiuixBasicComponent>
      </div>
    </MiuixCard>

    <MiuixSmallTitle :text="t('status.sysInfoTitle')" />
    <MiuixCard class="ex-card">
      <MiuixBasicComponent
        :title="t('status.kernel')"
        :summary="kernel_version"
      />
      <MiuixBasicComponent :title="t('status.selinux')" :summary="selinux" />
    </MiuixCard>

    <div v-if="activeMounts.length > 0">
      <MiuixSmallTitle :text="t('status.activePartitions')" />
      <MiuixCard class="ex-card">
        <div class="ex-basic-row ex-grow">
          <MiuixBasicComponent
            v-for="(mount, index) in activeMounts"
            :key="index"
            :title="mount"
          />
        </div>
      </MiuixCard>
    </div>
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
.ex-basic-row {
  display: flex;
  gap: 12px;
}
.ex-grow {
  flex: 1;
}
</style>
