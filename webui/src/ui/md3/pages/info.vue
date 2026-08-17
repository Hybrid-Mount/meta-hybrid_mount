<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { REPOSITORY_URL } from "../../../lib/constants";
import { sysStore } from "../../../lib/stores/sysStore";
import { API } from "../../../lib/api";

const { t } = useI18n();

interface Contributor {
  login: string;
  avatar_url: string;
  html_url: string;
  bio?: string;
}

const contributors = ref<Contributor[]>([]);
const failed = ref(false);

async function loadContributors(): Promise<void> {
  try {
    const response = await fetch(
      "https://api.github.com/repos/Hybrid-Mount/meta-hybrid_mount/contributors",
    );
    if (!response.ok) throw new Error(String(response.status));
    contributors.value = ((await response.json()) as Contributor[]).slice(0, 20);
  } catch {
    failed.value = true;
  }
}

function openRepository(): void {
  API.openLink(REPOSITORY_URL);
}

onMounted(async () => {
  await sysStore.ensureStatusLoaded();
  await loadContributors();
});
</script>

<template>
  <div class="page">
    <div class="md3-card">
      <h4>{{ t("content.welcome") }}</h4>
      <p>{{ t("content.tagline") }}</p>
      <p>
        <b>{{ t("info.version") }}</b
        >: {{ sysStore.version }}
      </p>
      <p>
        <b>{{ t("info.license") }}</b
        >: Core GPL-3.0-only · WebUI Apache-2.0
      </p>
      <p>
        <b>{{ t("status.mountSource") }}</b
        >:
        {{ sysStore.installState?.mount_source ?? "-" }}
      </p>
      <div class="md3-actions">
        <button class="md3-button" @click="openRepository">
          {{ t("info.projectLink") }}
        </button>
      </div>
    </div>

    <div class="md3-card">
      <h4>{{ t("info.installTitle") }}</h4>
      <p>
        <b>{{ t("info.selfModule") }}</b
        >:
        {{ sysStore.installState?.self_module ? "✓" : "✗" }}
      </p>
      <p>
        <b>{{ t("info.binary") }}</b
        >: {{ sysStore.installState?.binary ? "✓" : "✗" }}
      </p>
      <p>
        <b>{{ t("info.configExists") }}</b
        >:
        {{ sysStore.installState?.config_exists ? "✓" : "✗" }}
      </p>
      <p>
        <b>{{ t("info.overlaySupported") }}</b
        >:
        {{ sysStore.installState?.overlay_supported ? "✓" : "✗" }}
      </p>
      <p>
        <b>{{ t("status.compatible") }}</b
        >:
        {{ sysStore.installState?.compatible ? "✓" : "✗" }}
      </p>
    </div>

    <div class="md3-card">
      <h4>{{ t("info.contributors") }}</h4>
      <p v-if="failed">
        {{ t("info.loadFail") }}
      </p>
      <p v-for="contributor in contributors" :key="contributor.login">
        <b>{{ contributor.login }}</b> — {{ contributor.bio || t("info.noBio") }}
      </p>
    </div>
  </div>
</template>
