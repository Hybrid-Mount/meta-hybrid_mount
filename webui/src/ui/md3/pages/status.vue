<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { sysStore } from "../../../lib/stores/sysStore";
import { moduleStore } from "../../../lib/stores/moduleStore";
import { configStore } from "../../../lib/stores/configStore";

const { t } = useI18n();

const mountedCount = computed(
  () => moduleStore.modules.filter((module) => module.is_mounted).length,
);

onMounted(async () => {
  await Promise.all([
    sysStore.loadStatus(),
    moduleStore.loadModules(),
    configStore.loadConfig(),
  ]);
});
</script>

<template>
  <div class="page">
    <div class="md3-card">
      <h4>{{ t("content.welcome") }}</h4>
      <p>{{ sysStore.device.model }} · {{ t("content.tagline") }}</p>
    </div>

    <div class="md3-row">
      <div class="md3-card">
        <h4>{{ t("status.moduleActive") }}</h4>
        <p>{{ mountedCount }}</p>
      </div>
      <div class="md3-card">
        <h4>{{ t("status.mountSource") }}</h4>
        <p>{{ configStore.config.mountsource }}</p>
      </div>
    </div>

    <div class="md3-card">
      <h4>{{ t("status.backendTitle") }}</h4>
      <p>
        <b>{{ t("status.storageMode") }}</b
        >: {{ sysStore.state?.storage_mode ?? "-" }}
      </p>
      <p>
        <b>{{ t("status.overlayModules") }}</b
        >: {{ sysStore.state?.overlay_modules.length ?? 0 }}
      </p>
      <p>
        <b>{{ t("status.magicModules") }}</b
        >: {{ sysStore.state?.magic_modules.length ?? 0 }}
      </p>
      <p>
        <b>{{ t("status.activeMounts") }}</b
        >: {{ sysStore.state?.active_mounts.join(", ") || t("status.notReady") }}
      </p>
    </div>

    <div class="md3-card">
      <h4>{{ t("status.sysInfoTitle") }}</h4>
      <p>
        <b>{{ t("status.modelLabel") }}</b
        >: {{ sysStore.device.model }}
      </p>
      <p>
        <b>{{ t("status.androidLabel") }}</b
        >: {{ sysStore.device.android }}
      </p>
      <p>
        <b>{{ t("status.kernelLabel") }}</b
        >: {{ sysStore.systemInfo.kernel }}
      </p>
      <p>
        <b>{{ t("status.selinuxLabel") }}</b
        >: {{ sysStore.systemInfo.selinux }}
      </p>
    </div>

    <div class="md3-card">
      <h4>{{ t("status.installTitle") }}</h4>
      <p v-if="sysStore.installState">
        <b>{{ t("status.compatible") }}</b
        >:
        {{ sysStore.installState.compatible ? "✓" : "✗" }}
      </p>
      <div class="md3-actions">
        <button class="md3-button" @click="sysStore.loadStatus()">
          {{ t("common.refresh") }}
        </button>
      </div>
    </div>
  </div>
</template>
