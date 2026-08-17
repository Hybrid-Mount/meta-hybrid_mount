<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { MiuixCard, MiuixSmallTitle, MiuixBasicComponent, MiuixButton } from "miuix-vue";
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
const loadingContributors = ref(false);
const loadFailed = ref(false);

async function loadContributors(): Promise<void> {
  if (contributors.value.length) return;
  loadingContributors.value = true;
  loadFailed.value = false;
  try {
    const response = await fetch(
      "https://api.github.com/repos/Hybrid-Mount/meta-hybrid_mount/contributors",
    );
    if (!response.ok) throw new Error(String(response.status));
    const list = (await response.json()) as Contributor[];
    contributors.value = list.slice(0, 20);
  } catch {
    loadFailed.value = true;
  } finally {
    loadingContributors.value = false;
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
    <MiuixCard class="card">
      <MiuixBasicComponent
        :title="t('content.welcome')"
        :summary="t('content.tagline')"
      />
      <MiuixBasicComponent :title="t('info.version')" :summary="sysStore.version" />
      <MiuixBasicComponent
        :title="t('info.license')"
        summary="Core: GPL-3.0-only · WebUI: Apache-2.0"
      />
      <MiuixBasicComponent
        :title="t('status.mountSource')"
        :summary="sysStore.installState?.mount_source ?? '-'"
      />
      <template #footer>
        <MiuixButton @click="openRepository">
          {{ t("info.projectLink") }}
        </MiuixButton>
      </template>
    </MiuixCard>

    <MiuixSmallTitle :text="t('info.installTitle')" />
    <MiuixCard class="card">
      <MiuixBasicComponent
        :title="t('info.selfModule')"
        :summary="sysStore.installState?.self_module ? '✓' : '✗'"
      />
      <MiuixBasicComponent
        :title="t('info.binary')"
        :summary="sysStore.installState?.binary ? '✓' : '✗'"
      />
      <MiuixBasicComponent
        :title="t('info.configExists')"
        :summary="sysStore.installState?.config_exists ? '✓' : '✗'"
      />
      <MiuixBasicComponent
        :title="t('info.overlaySupported')"
        :summary="sysStore.installState?.overlay_supported ? '✓' : '✗'"
      />
      <MiuixBasicComponent
        :title="t('status.compatible')"
        :summary="sysStore.installState?.compatible ? '✓' : '✗'"
      />
    </MiuixCard>

    <MiuixSmallTitle :text="t('info.contributors')" />
    <MiuixCard class="card">
      <MiuixBasicComponent v-if="loadingContributors" :title="t('info.loading')" />
      <MiuixBasicComponent v-else-if="loadFailed" :title="t('info.loadFail')" />
      <MiuixBasicComponent
        v-for="contributor in contributors"
        :key="contributor.login"
        :title="contributor.login"
        :summary="contributor.bio || t('info.noBio')"
      />
    </MiuixCard>
  </div>
</template>
