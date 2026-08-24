<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import {
  MiuixCard,
  MiuixSmallTitle,
  MiuixBasicComponent,
  MiuixText,
  MiuixIcon,
  MiuixIconButton,
} from "miuix-vue";
import { Refresh } from "miuix-vue/icons";
import { uiStore } from "../../../lib/stores/uiStore";
import { sysStore } from "../../../lib/stores/sysStore";
import { moduleStore } from "../../../lib/stores/moduleStore";
import { configStore } from "../../../lib/stores/configStore";

const { t } = useI18n();

const state = computed(() => sysStore.state);
const mountedCount = computed(
  () => moduleStore.modules.filter((module) => module.is_mounted).length,
);

function handleSetNav(index: number): void {
  if (!sysStore.loading) uiStore.setNavindex(index);
}

onMounted(async () => {
  await Promise.all([
    sysStore.loadStatus(),
    moduleStore.loadModules(),
    configStore.ensureConfigLoaded(),
  ]);
});
</script>

<template>
  <div class="page">
    <MiuixCard class="card">
      <MiuixBasicComponent
        :title="t('status.modelLabel')"
        :summary="sysStore.device.model || '-'"
      />
    </MiuixCard>

    <div class="card-row">
      <MiuixCard show-indication press-feedback="sink" class="grow">
        <MiuixBasicComponent
          :title="t('status.moduleActive')"
          clickable
          @click="handleSetNav(2)"
        >
          <template #end>
            <MiuixText>{{ mountedCount }}</MiuixText>
          </template>
        </MiuixBasicComponent>
      </MiuixCard>
      <MiuixCard show-indication press-feedback="sink" class="grow">
        <MiuixBasicComponent
          :title="t('status.mountSource')"
          clickable
          @click="handleSetNav(1)"
        >
          <template #end>
            <MiuixText>{{ configStore.config.mountsource }}</MiuixText>
          </template>
        </MiuixBasicComponent>
      </MiuixCard>
    </div>

    <MiuixSmallTitle :text="t('status.backendTitle')" />
    <MiuixCard class="card">
      <MiuixBasicComponent
        :title="t('status.storageMode')"
        :summary="state?.storage_mode ?? '-'"
      />
      <MiuixBasicComponent
        :title="t('status.overlayModules')"
        :summary="state?.overlay_modules.join(', ') || '0'"
      />
      <MiuixBasicComponent
        :title="t('status.magicModules')"
        :summary="state?.magic_modules.join(', ') || '0'"
      />
      <MiuixBasicComponent
        :title="t('status.activeMounts')"
        :summary="state?.active_mounts.join(', ') || t('status.notReady')"
      />
    </MiuixCard>

    <MiuixSmallTitle :text="t('status.sysInfoTitle')" />
    <MiuixCard class="card">
      <MiuixBasicComponent
        :title="t('status.kernelLabel')"
        :summary="sysStore.systemInfo.kernel"
      />
      <MiuixBasicComponent
        :title="t('status.selinuxLabel')"
        :summary="sysStore.systemInfo.selinux"
      />
      <MiuixBasicComponent
        :title="t('status.androidLabel')"
        :summary="sysStore.device.android"
      />
    </MiuixCard>

    <div class="actions">
      <MiuixIconButton
        :title="t('common.refresh')"
        :aria-label="t('common.refresh')"
        :disabled="sysStore.loading"
        @click="sysStore.loadStatus()"
      >
        <MiuixIcon :icon="Refresh" :size="22" />
      </MiuixIconButton>
    </div>
  </div>
</template>

<style scoped>
.actions {
  display: flex;
  justify-content: flex-end;
  margin: 12px 0;
}
</style>
