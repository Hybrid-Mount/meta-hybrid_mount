<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { sysStore } from "../../../lib/stores/sysStore";
import { moduleStore } from "../../../lib/stores/moduleStore";
import { configStore } from "../../../lib/stores/configStore";
import {
  activeMountState,
  groupActiveMounts,
  uniqueActiveMounts,
} from "../../../lib/statusMounts";
import Md3BottomActions from "../components/Md3BottomActions.vue";
import { ICONS } from "../icons";

const { t } = useI18n();
const rebootOpen = ref(false);

const mountedCount = computed(
  () => moduleStore.modules.filter((module) => module.is_mounted).length,
);
const overlayCount = computed(() => sysStore.state?.mode_stats.overlayfs ?? 0);
const magicCount = computed(() => sysStore.state?.mode_stats.magicmount ?? 0);
const vfsCount = computed(() => sysStore.state?.mode_stats.vfs ?? 0);
const overlayMountCount = computed(
  () => sysStore.state?.mount_stats.overlayfs_mounts ?? 0,
);
const magicFileMountCount = computed(
  () => sysStore.state?.mount_stats.files_mounted ?? 0,
);
const magicSymlinkCount = computed(
  () => sysStore.state?.mount_stats.symlinks_created ?? 0,
);
const modeTotal = computed(() => overlayCount.value + magicCount.value + vfsCount.value);
const overlayWidth = computed(() =>
  modeTotal.value ? `${(overlayCount.value / modeTotal.value) * 100}%` : "0%",
);
const magicWidth = computed(() =>
  modeTotal.value ? `${(magicCount.value / modeTotal.value) * 100}%` : "0%",
);
const vfsWidth = computed(() =>
  modeTotal.value ? `${(vfsCount.value / modeTotal.value) * 100}%` : "0%",
);
const activeMounts = computed(() =>
  uniqueActiveMounts(sysStore.state?.active_mounts ?? []),
);
const activeMountGroups = computed(() => groupActiveMounts(activeMounts.value));
const activeMountStatus = computed(() =>
  activeMountState(sysStore.state, activeMounts.value),
);
const failureSummary = computed(() => {
  const state = sysStore.state;
  if (!state) return null;
  if (state.failure_reason) return state.failure_reason;
  if (state.failed_stage) return `${t("status.abnormal")}: ${state.failed_stage}`;
  if (state.mount_stats.failed_mounts > 0) {
    return t("status.mountFailures", { count: state.mount_stats.failed_mounts });
  }
  if (state.rollback_status === "incomplete" || state.rollback_status === "unverified") {
    return `${t("status.abnormal")}: rollback ${state.rollback_status}`;
  }
  if (state.state_load.kind === "corrupt" || state.state_load.kind === "io_error") {
    return state.state_load.detail || t("status.loadError");
  }
  return null;
});

async function refresh(): Promise<void> {
  await Promise.all([
    sysStore.loadStatus(),
    moduleStore.loadModules(),
    configStore.ensureConfigLoaded(),
  ]);
}

async function rebootSystem(): Promise<void> {
  rebootOpen.value = false;
  await sysStore.rebootDevice();
}

onMounted(refresh);
</script>

<template>
  <div class="page">
    <div class="dashboard-grid">
      <section v-if="failureSummary" class="failure-card" role="alert">
        <strong>{{ t("status.abnormal") }}</strong>
        <span>{{ failureSummary }}</span>
        <code v-if="sysStore.state?.leftover_mount_targets.length">
          {{ sysStore.state.leftover_mount_targets.join(", ") }}
        </code>
      </section>
      <section class="hero-card">
        <div v-if="sysStore.loading" class="skeleton-col">
          <md-circular-progress indeterminate />
        </div>
        <template v-else>
          <div class="hero-content">
            <span class="hero-label">{{ t("status.backendTitle") }}</span>
            <span class="hero-value">
              {{ (sysStore.state?.storage_mode || "-").toUpperCase() }}
            </span>
          </div>
        </template>
      </section>

      <div class="metrics-row">
        <section class="metric-card">
          <div class="metric-icon-bg">
            <svg viewBox="0 0 24 24"><path :d="ICONS.modules" /></svg>
          </div>
          <span class="metric-value">{{ mountedCount }}</span>
          <span class="metric-label">{{ t("status.moduleActive") }}</span>
        </section>
        <section class="metric-card">
          <div class="metric-icon-bg">
            <svg viewBox="0 0 24 24"><path :d="ICONS.ksu" /></svg>
          </div>
          <span class="metric-value">
            {{ sysStore.installState?.mount_source || "-" }}
          </span>
          <span class="metric-label">{{ t("status.mountSource") }}</span>
        </section>
      </div>

      <section class="mode-stats-card">
        <div class="card-title">{{ t("status.modeStats") }}</div>
        <div class="stats-bar-container" :aria-label="t('status.modeStats')">
          <div class="bar-segment bar-overlay" :style="{ width: overlayWidth }" />
          <div class="bar-segment bar-magic" :style="{ width: magicWidth }" />
          <div class="bar-segment bar-vfs" :style="{ width: vfsWidth }" />
        </div>
        <div class="stats-legend">
          <div class="legend-item">
            <span class="legend-dot dot-overlay" />
            <span>
              {{ t("config.modeOverlay") }}:
              {{ t("status.moduleCount", { count: overlayCount }) }} ·
              {{ t("status.mountCount", { count: overlayMountCount }) }}
            </span>
          </div>
          <div class="legend-item">
            <span class="legend-dot dot-magic" />
            <span>
              {{ t("config.modeMagic") }}:
              {{ t("status.moduleCount", { count: magicCount }) }} ·
              {{
                t("status.magicOperationCount", {
                  files: magicFileMountCount,
                  symlinks: magicSymlinkCount,
                })
              }}
            </span>
          </div>
          <div class="legend-item">
            <span class="legend-dot dot-vfs" />
            <span>
              {{ t("config.modeVfs") }}:
              {{ t("status.moduleCount", { count: vfsCount }) }}
            </span>
          </div>
        </div>
      </section>

      <section class="info-card">
        <div class="card-title">{{ t("status.sysInfoTitle") }}</div>
        <div class="info-row">
          <span class="info-key">{{ t("status.modelLabel") }}</span>
          <span class="info-val">{{ sysStore.device.model || "-" }}</span>
        </div>
        <div class="info-row">
          <span class="info-key">{{ t("status.androidLabel") }}</span>
          <span class="info-val">{{ sysStore.device.android || "-" }}</span>
        </div>
        <div class="info-row">
          <span class="info-key">{{ t("status.kernelLabel") }}</span>
          <span class="info-val">{{ sysStore.systemInfo.kernel || "-" }}</span>
        </div>
        <div class="info-row">
          <span class="info-key">{{ t("status.selinuxLabel") }}</span>
          <span class="info-val">{{ sysStore.systemInfo.selinux || "-" }}</span>
        </div>

        <div class="card-title card-title-spaced">{{ t("status.activeMounts") }}</div>
        <div class="partition-list">
          <span v-if="activeMountStatus === 'not-ready'" class="partition-chip">
            {{ t("status.notReady") }}
          </span>
          <span v-else-if="activeMountStatus === 'empty'" class="partition-chip">
            {{ t("status.noActiveMounts") }}
          </span>
          <template v-else>
            <span
              v-for="group in activeMountGroups"
              :key="group.root"
              class="partition-chip active"
            >
              {{ group.root }} ·
              {{ t("status.mountTargetCount", { count: group.count }) }}
            </span>
          </template>
        </div>
        <details v-if="activeMountStatus === 'active'" class="mount-details">
          <summary>
            {{ t("status.mountDetails", { count: activeMounts.length }) }}
          </summary>
          <div class="mount-path-list">
            <code v-for="mount in activeMounts" :key="mount">{{ mount }}</code>
          </div>
        </details>
      </section>
    </div>

    <Md3BottomActions>
      <md-filled-tonal-icon-button
        class="destructive-action"
        :title="t('common.reboot')"
        :aria-label="t('common.reboot')"
        @click="rebootOpen = true"
      >
        <md-icon
          ><svg viewBox="0 0 24 24"><path :d="ICONS.power" /></svg
        ></md-icon>
      </md-filled-tonal-icon-button>
      <md-filled-tonal-icon-button
        :disabled="sysStore.loading"
        :title="t('common.refresh')"
        :aria-label="t('common.refresh')"
        @click="refresh"
      >
        <md-icon
          ><svg viewBox="0 0 24 24"><path :d="ICONS.refresh" /></svg
        ></md-icon>
      </md-filled-tonal-icon-button>
    </Md3BottomActions>

    <md-dialog :open="rebootOpen" @closed="rebootOpen = false">
      <div slot="headline">{{ t("common.rebootTitle") }}</div>
      <div slot="content">{{ t("common.rebootConfirm") }}</div>
      <div slot="actions">
        <md-text-button @click="rebootOpen = false">{{
          t("common.cancel")
        }}</md-text-button>
        <md-text-button @click="rebootSystem">{{ t("common.reboot") }}</md-text-button>
      </div>
    </md-dialog>
  </div>
</template>
