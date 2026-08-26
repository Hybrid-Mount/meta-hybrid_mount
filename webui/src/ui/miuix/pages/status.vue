<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
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
const statusCheckFinished = ref(sysStore.hasLoaded);
const mountedCount = computed(
  () => moduleStore.modules.filter((module) => module.is_mounted).length,
);
const overlayMountCount = computed(() => state.value?.mount_stats.overlayfs_mounts ?? 0);
const magicMountCount = computed(
  () =>
    (state.value?.mount_stats.files_mounted ?? 0) +
    (state.value?.mount_stats.symlinks_created ?? 0),
);
const activeMounts = computed(() => [...new Set(state.value?.active_mounts ?? [])]);
const statusKind = computed<"checking" | "normal" | "abnormal">(() => {
  if (!statusCheckFinished.value) return "checking";

  const installState = sysStore.installState;
  if (
    !state.value ||
    !installState?.installed ||
    !installState.compatible ||
    state.value.timestamp <= 0 ||
    state.value.mount_stats.failed_mounts > 0
  ) {
    return "abnormal";
  }

  return "normal";
});
const statusTitle = computed(() => {
  if (statusKind.value === "checking") return t("status.checking");
  if (statusKind.value === "normal") return t("status.working");
  return t("status.abnormal");
});
const statusSummary = computed(() => {
  if (statusKind.value === "checking") return t("status.checkingSummary");
  if (!sysStore.installState?.installed || !sysStore.installState?.compatible) {
    return t("status.installIncomplete");
  }
  if (!state.value || state.value.timestamp <= 0) return t("status.notReady");
  if (state.value.mount_stats.failed_mounts > 0) {
    return t("status.mountFailures", {
      count: state.value.mount_stats.failed_mounts,
    });
  }
  return t("status.workingVersion", { version: sysStore.version });
});
function handleSetNav(index: number): void {
  if (!sysStore.loading) uiStore.setNavindex(index);
}

onMounted(async () => {
  await Promise.all([
    sysStore.loadStatus(),
    moduleStore.loadModules(),
    configStore.ensureConfigLoaded(),
  ]);
  statusCheckFinished.value = true;
});
</script>

<template>
  <div class="page">
    <MiuixCard class="status-banner">
      <div class="status-banner__content" :class="`status-banner--${statusKind}`">
        <div class="status-banner__copy">
          <MiuixText class="status-banner__title">{{ statusTitle }}</MiuixText>
          <MiuixText class="status-banner__summary">{{ statusSummary }}</MiuixText>
        </div>
        <MiuixText v-if="state?.storage_mode" class="status-banner__mode">
          {{ state.storage_mode.toUpperCase() }}
        </MiuixText>
        <div class="status-banner__symbol" aria-hidden="true" />
      </div>
    </MiuixCard>

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
            <MiuixText>
              {{ sysStore.installState?.mount_source || configStore.config.mountsource }}
            </MiuixText>
          </template>
        </MiuixBasicComponent>
      </MiuixCard>
    </div>

    <MiuixSmallTitle :text="t('status.backendTitle')" />
    <MiuixCard class="card backend-card">
      <MiuixBasicComponent
        :title="t('status.storageMode')"
        :summary="state?.storage_mode ?? '-'"
      />
      <MiuixBasicComponent
        class="backend-row"
        :title="t('status.overlayModules')"
        :summary="state?.overlay_modules.join(', ') || '0'"
      >
        <template #end>
          <MiuixText class="backend-row__count">
            {{ t("status.mountCount", { count: overlayMountCount }) }}
          </MiuixText>
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        class="backend-row"
        :title="t('status.magicModules')"
        :summary="state?.magic_modules.join(', ') || '0'"
      >
        <template #end>
          <MiuixText class="backend-row__count">
            {{ t("status.mountCount", { count: magicMountCount }) }}
          </MiuixText>
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        :title="t('status.activeMounts')"
        :summary="activeMounts.join(', ') || t('status.notReady')"
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
.status-banner {
  margin: 0 12px 12px;
}

.status-banner :deep(.m-card) {
  --m-card-color: transparent;
  border-radius: var(--m-radius-md, 16px);
}

.status-banner__content {
  box-sizing: border-box;
  min-height: 144px;
  padding: 14px 16px;
  position: relative;
  overflow: hidden;
  color: var(--m-color-on-surface-container);
  background: var(--m-color-surface-container-high);
}

.status-banner--normal {
  background: var(--status-normal-container);
  color: var(--status-normal-content);
}

.status-banner--normal .status-banner__symbol {
  color: var(--status-normal-accent);
}

.status-banner--abnormal {
  background: var(--status-abnormal-container);
  color: var(--status-abnormal-content);
}

.status-banner--abnormal .status-banner__symbol {
  color: var(--status-abnormal-accent);
}

.status-banner--checking {
  background: var(--status-checking-container);
  color: var(--status-checking-content);
}

.status-banner--checking .status-banner__symbol {
  color: var(--status-checking-accent);
}

.status-banner__copy {
  max-width: calc(100% - 92px);
  display: flex;
  flex-direction: column;
  gap: 1px;
  position: relative;
  z-index: 1;
}

.status-banner__title {
  font-size: 22px;
  font-weight: 600;
}

.status-banner__summary {
  font-size: 15px;
  font-weight: 500;
  opacity: 0.72;
}

.status-banner__mode {
  position: absolute;
  bottom: 10px;
  left: 16px;
  font-size: 16px;
  font-weight: 500;
}

.status-banner__symbol {
  box-sizing: border-box;
  width: 110px;
  height: 110px;
  border: 8px solid currentColor;
  border-radius: 50%;
  position: absolute;
  right: -27px;
  bottom: -31px;
  opacity: 0.8;
}

.status-banner--normal .status-banner__symbol::before,
.status-banner--normal .status-banner__symbol::after,
.status-banner--abnormal .status-banner__symbol::before,
.status-banner--abnormal .status-banner__symbol::after {
  content: "";
  height: 8px;
  border-radius: 999px;
  background: currentColor;
  position: absolute;
  transform-origin: left center;
}

.status-banner--normal .status-banner__symbol::before {
  width: 30px;
  left: 21px;
  top: 54px;
  transform: rotate(45deg);
}

.status-banner--normal .status-banner__symbol::after {
  width: 55px;
  left: 40px;
  top: 70px;
  transform: rotate(-45deg);
}

.status-banner--abnormal .status-banner__symbol::before,
.status-banner--abnormal .status-banner__symbol::after {
  width: 58px;
  left: 25px;
  top: 47px;
  transform-origin: center;
}

.status-banner--abnormal .status-banner__symbol::before {
  transform: rotate(45deg);
}

.status-banner--abnormal .status-banner__symbol::after {
  transform: rotate(-45deg);
}

.status-banner--checking .status-banner__symbol {
  border-top-color: transparent;
  animation: status-spin 1.4s linear infinite;
}

.actions {
  display: flex;
  justify-content: flex-end;
  margin: 12px 0;
}

.backend-card :deep(.m-basic-component__center) {
  overflow: hidden;
}

.backend-card :deep(.m-basic-component__center .m-text) {
  max-width: 100%;
  overflow-wrap: anywhere;
}

.backend-row :deep(.m-basic-component__row) {
  align-items: flex-start;
}

.backend-row :deep(.m-basic-component__end) {
  align-self: flex-start;
  padding-top: 2px;
}

.backend-row__count {
  white-space: nowrap;
}

@keyframes status-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .status-banner--checking .status-banner__symbol {
    animation: none;
  }
}
</style>
