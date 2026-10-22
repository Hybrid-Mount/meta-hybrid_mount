<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  MiuixIcon,
  MiuixCard,
  MiuixSmallTitle,
  MiuixBasicComponent,
  MiuixText,
  MiuixButton,
  MiuixDialog,
} from "miuix-vue";
import { Settings } from "miuix-vue/icons";
import { Motion, AnimatePresence } from "motion-v";
import Android from "vue-material-design-icons/Android.vue";
import Security from "vue-material-design-icons/Security.vue";
import DeveloperBoard from "vue-material-design-icons/DeveloperBoard.vue";
import Cellphone from "vue-material-design-icons/Cellphone.vue";

import StatusCard from "../components/StatusCard.vue";
import StatusErrorBanner from "../components/StatusErrorBanner.vue";
import { uiStore } from "../../../lib/stores/uiStore";
import { sysStore } from "../../../lib/stores/sysStore";
import { moduleStore } from "../../../lib/stores/moduleStore";
import { configStore } from "../../../lib/stores/configStore";
import {
  activeMountState,
  backendDisplayKeys,
  collectStatusErrors,
  groupActiveMounts,
  statusErrorDetails,
  statusErrorRows,
  statusFailureSummary,
  storageModeKey,
  uniqueActiveMounts,
} from "../../../lib/statusMounts";
import type { BannerAction } from "../../../lib/statusMounts";

const { t } = useI18n();
const expandSpring = { type: "spring" as const, stiffness: 400, damping: 40 };
const guardOpen = ref(false);
const guardBusy = ref(false);

const state = computed(() => sysStore.state);
const statusCheckFinished = ref(sysStore.hasLoaded);
const mountedCount = computed(
  () => moduleStore.modules.filter((module) => module.is_mounted).length,
);
const overlayMountCount = computed(() => state.value?.mount_stats.overlayfs_mounts ?? 0);
const showoverlaymodule = ref(false);
const magicFileMountCount = computed(() => state.value?.mount_stats.files_mounted ?? 0);
const magicSymlinkCount = computed(() => state.value?.mount_stats.symlinks_created ?? 0);
const showmagicmodule = ref(false);
const showvfsmodule = ref(false);
const vfsCount = computed(() =>
  sysStore.vfsSupported ? (state.value?.mode_stats.vfs ?? 0) : 0,
);
const expandMountPath = ref(false);
const activeMounts = computed(() => uniqueActiveMounts(state.value?.active_mounts ?? []));
// Name the backends that actually ran. A VFS-only boot created no Tmpfs/Ext4 staging,
// so it must read "VFS" instead of a storage mode that never started.
const backendSummary = computed(() => {
  const labels = backendDisplayKeys(state.value, sysStore.vfsSupported).map((key) =>
    t(key),
  );
  return labels.length > 0 ? labels.join(" · ") : "-";
});
const storageModeLabel = computed(() => {
  const key = storageModeKey(state.value);
  return key ? t(key) : "-";
});
const activeMountGroups = computed(() => groupActiveMounts(activeMounts.value));
const activeMountStatus = computed(() =>
  activeMountState(state.value, activeMounts.value),
);
const activeMountSummary = computed(() => {
  if (activeMountStatus.value === "not-ready") return t("status.notReady");
  if (activeMountStatus.value === "empty") return t("status.noActiveMounts");
  return activeMountGroups.value
    .map(
      (group) =>
        `${group.root} · ${t("status.mountTargetCount", { count: group.count })}`,
    )
    .join(", ");
});
const statusKind = computed<"checking" | "normal" | "abnormal">(() => {
  if (!statusCheckFinished.value) return "checking";

  const installState = sysStore.installState;
  if (
    !state.value ||
    !installState?.installed ||
    !installState.compatible ||
    state.value.timestamp <= 0 ||
    state.value.mount_stats.failed_mounts > 0 ||
    (sysStore.vfsSupported && state.value.vfs_error !== null) ||
    state.value.failed_stage !== null ||
    state.value.rollback_status === "incomplete" ||
    state.value.rollback_status === "unverified" ||
    state.value.state_load.kind === "corrupt" ||
    state.value.state_load.kind === "io_error"
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
  return (
    statusFailureSummary(
      state.value,
      {
        abnormal: t("status.abnormal"),
        loadError: t("status.loadError"),
        mountFailures: (count) => t("status.mountFailures", { count }),
      },
      sysStore.vfsSupported,
    ) ?? t("status.workingVersion", { version: sysStore.version })
  );
});
// Every problem this boot reported, not just the first one. An empty list keeps the banner off
// screen, so a healthy device never sees it.
const statusErrors = computed(() =>
  collectStatusErrors(state.value, {
    loadError: sysStore.loadError,
    moduleScanError: moduleStore.loadError,
  }),
);
const errorRows = computed(() =>
  statusErrorRows(statusErrorDetails(state.value, sysStore.vfsSupported), {
    stage: t("status.detailStage"),
    rollback: t("status.detailRollback"),
    version: t("status.detailVersion"),
  }),
);
function handleSetNav(index: number): void {
  if (!sysStore.loading) uiStore.setNavindex(index);
}

// Clearing a guard lets the next boot inject VFS rules again, so the banner offers it only
// while the device really reports a marker this build owns or cannot attribute.
const guardAction = computed<BannerAction | null>(() =>
  sysStore.clearableBootGuards.length > 0
    ? { label: t("status.guardAction"), busy: guardBusy.value }
    : null,
);

async function clearBootGuards(): Promise<void> {
  guardOpen.value = false;
  guardBusy.value = true;
  try {
    const cleared = await sysStore.clearBootGuards();
    uiStore.showToast(t("status.guardCleared", { count: cleared }));
  } catch {
    uiStore.showToast(t("status.guardClearFailed"));
  } finally {
    guardBusy.value = false;
  }
}

onMounted(async () => {
  await Promise.all([
    sysStore.loadStatus(),
    sysStore.loadBootGuards(),
    moduleStore.loadModules(),
    configStore.ensureConfigLoaded(),
  ]);
  statusCheckFinished.value = true;
});
</script>

<template>
  <div class="page">
    <StatusCard
      class="status-banner"
      :status="statusKind"
      :label="statusTitle"
      :summary="statusSummary"
      :description="backendSummary"
    ></StatusCard>

    <StatusErrorBanner
      :title="t('common.statusErrors')"
      :hint="t('common.statusErrorsHint')"
      :errors="statusErrors"
      :details="errorRows"
      :items-label="t('status.errorItems')"
      :action="guardAction"
      @action="guardOpen = true"
    />

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
              {{ sysStore.installState?.mount_source || "-" }}
            </MiuixText>
          </template>
        </MiuixBasicComponent>
      </MiuixCard>
    </div>

    <MiuixSmallTitle :text="t('status.backendTitle')" />
    <MiuixCard class="card backend-card">
      <MiuixBasicComponent :title="t('status.storageMode')" :summary="storageModeLabel">
        <template #start>
          <MiuixIcon :icon="Settings" weight="medium" />
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        class="backend-row"
        :title="t('status.overlayModules')"
        :summary="
          showoverlaymodule
            ? state?.overlay_modules.join(', ') || '0'
            : t('status.mountCount', { count: overlayMountCount })
        "
        clickable
        @click="showoverlaymodule = !showoverlaymodule"
      >
        <template #start>
          <MiuixIcon :icon="Settings" weight="medium" />
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        class="backend-row"
        :title="t('status.magicModules')"
        :summary="
          showmagicmodule
            ? state?.magic_modules.join(', ') || '0'
            : t('status.magicOperationCount', {
                files: magicFileMountCount,
                symlinks: magicSymlinkCount,
              })
        "
        clickable
        @click="showmagicmodule = !showmagicmodule"
      >
        <template #start>
          <MiuixIcon :icon="Settings" weight="medium" />
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        v-if="sysStore.vfsSupported"
        class="backend-row"
        :title="t('status.vfsModules')"
        :summary="
          showvfsmodule
            ? state?.vfs_modules.join(', ') || '0'
            : t('status.moduleCount', { count: vfsCount })
        "
        clickable
        @click="showvfsmodule = !showvfsmodule"
      >
        <template #start>
          <MiuixIcon :icon="Settings" weight="medium" />
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        :title="t('status.activeMounts')"
        :summary="activeMountSummary"
        clickable
        @click="expandMountPath = !expandMountPath"
      >
        <template #start>
          <MiuixIcon :icon="Settings" weight="medium" />
        </template>
      </MiuixBasicComponent>
      <AnimatePresence :initial="false">
        <Motion
          v-if="activeMountStatus === 'active' && expandMountPath"
          class="mount-details"
          :initial="{ height: 0, opacity: 0 }"
          :animate="{ height: 'auto', opacity: 1 }"
          :exit="{ height: 0, opacity: 0 }"
          :transition="expandSpring"
        >
          <div class="mount-path-list">
            <code v-for="mount in activeMounts" :key="mount">{{ mount }}</code>
          </div>
        </Motion>
      </AnimatePresence>
    </MiuixCard>

    <MiuixSmallTitle :text="t('status.sysInfoTitle')" />
    <MiuixCard class="card">
      <MiuixBasicComponent
        :title="t('status.modelLabel')"
        :summary="sysStore.device.model || '-'"
      >
        <template #start>
          <Cellphone />
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        :title="t('status.kernelLabel')"
        :summary="sysStore.systemInfo.kernel"
      >
        <template #start>
          <DeveloperBoard />
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        :title="t('status.selinuxLabel')"
        :summary="sysStore.systemInfo.selinux"
      >
        <template #start>
          <Security />
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent
        :title="t('status.androidLabel')"
        :summary="sysStore.device.android"
      >
        <template #start>
          <Android />
        </template>
      </MiuixBasicComponent>
    </MiuixCard>
    <MiuixDialog
      v-model="guardOpen"
      :title="t('status.guardDialogTitle')"
      :summary="t('status.guardDialogBody')"
      @close="guardOpen = false"
    >
      <template #default="{ close }">
        <div class="guard-dialog-actions">
          <MiuixButton class="grow" @click="close">
            {{ t("common.cancel") }}
          </MiuixButton>
          <MiuixButton class="grow" type="primary" @click="clearBootGuards">
            {{ t("status.guardAction") }}
          </MiuixButton>
        </div>
      </template>
    </MiuixDialog>
  </div>
</template>

<style scoped>
/* The banner is a plain MiuixCard (see components/StatusCard.vue), so it takes
 * the same 12px list inset as every other card. */
.status-banner {
  margin: 0 12px 12px;
}

.status-banner :deep(.m-card) {
  border-radius: var(--m-radius-md, 16px);
}

.guard-dialog-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.guard-dialog-actions .grow {
  flex: 1;
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

.mount-details {
  margin: 0 16px 12px;
  color: var(--m-color-on-surface-variant);
}

.mount-path-list {
  display: grid;
  gap: 4px;
  max-height: 180px;
  overflow: auto;
}

.mount-path-list code {
  font-size: 12px;
  overflow-wrap: anywhere;
}
</style>
