<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import DynamicLogo from "../../../components/DynamicLogo.vue";
import { REPOSITORY_URL, TELEGRAM_URL } from "../../../lib/constants";
import {
  loadGitHubContributors,
  type GitHubContributor,
} from "../../../lib/contributors";
import { sysStore } from "../../../lib/stores/sysStore";
import { API } from "../../../lib/api";
import Md3BottomActions from "../components/Md3BottomActions.vue";
import { ICONS } from "../icons";

const { t } = useI18n();

const contributors = ref<GitHubContributor[]>([]);
const loading = ref(false);
const failed = ref(false);

async function loadContributors(): Promise<void> {
  loading.value = true;
  failed.value = false;
  try {
    contributors.value = await loadGitHubContributors();
  } catch {
    failed.value = true;
  } finally {
    loading.value = false;
  }
}

function statusText(value: boolean | undefined): string {
  return value ? "✓" : "—";
}

onMounted(async () => {
  await sysStore.ensureStatusLoaded();
  await loadContributors();
});
</script>

<template>
  <div class="info-container">
    <section class="project-header">
      <div class="app-logo" aria-hidden="true">
        <DynamicLogo />
      </div>
      <div class="app-name">{{ t("content.welcome") }}</div>
      <div class="app-version">v{{ sysStore.version }}</div>
    </section>

    <section class="config-card">
      <div class="card-header">
        <span class="card-icon">
          <md-icon
            ><svg viewBox="0 0 24 24"><path :d="ICONS.description" /></svg
          ></md-icon>
        </span>
        <span class="card-text">
          <span class="card-title">{{ t("info.installTitle") }}</span>
          <span class="card-desc"
            >{{ t("info.license") }}: GPL-3.0-only / Apache-2.0</span
          >
        </span>
      </div>
      <div class="setting-list">
        <div class="list-item">
          <span class="list-text"
            ><span class="list-title">{{ t("info.selfModule") }}</span></span
          >
          <strong>{{ statusText(sysStore.installState?.self_module) }}</strong>
        </div>
        <div class="item-separator" />
        <div class="list-item">
          <span class="list-text"
            ><span class="list-title">{{ t("info.binary") }}</span></span
          >
          <strong>{{ statusText(sysStore.installState?.binary) }}</strong>
        </div>
        <div class="item-separator" />
        <div class="list-item">
          <span class="list-text"
            ><span class="list-title">{{ t("info.configExists") }}</span></span
          >
          <strong>{{ statusText(sysStore.installState?.config_exists) }}</strong>
        </div>
        <div class="item-separator" />
        <div class="list-item">
          <span class="list-text"
            ><span class="list-title">{{ t("info.overlaySupported") }}</span></span
          >
          <strong>{{ statusText(sysStore.installState?.overlay_supported) }}</strong>
        </div>
      </div>
    </section>

    <section class="contributors-section">
      <h2 class="section-title">{{ t("info.contributors") }}</h2>
      <div v-if="loading" class="loading-container">
        <md-circular-progress indeterminate />
        <span>{{ t("info.loading") }}</span>
      </div>
      <p v-else-if="failed" class="error-message">{{ t("info.loadFail") }}</p>
      <div v-else class="contributors-list-vue">
        <button
          v-for="contributor in contributors"
          :key="contributor.login"
          type="button"
          class="contributor-link-vue"
          @click="API.openLink(contributor.html_url)"
        >
          <img :src="contributor.avatar_url" alt="" loading="lazy" />
          <span class="contributor-copy-vue">
            <strong>{{ contributor.name || contributor.login }}</strong>
            <span
              >@{{ contributor.login }} · {{ contributor.bio || t("info.noBio") }}</span
            >
          </span>
        </button>
      </div>
    </section>

    <Md3BottomActions>
      <md-filled-tonal-icon-button
        :title="t('info.projectLink')"
        :aria-label="t('info.projectLink')"
        @click="API.openLink(REPOSITORY_URL)"
      >
        <md-icon
          ><svg viewBox="0 0 24 24"><path :d="ICONS.github" /></svg
        ></md-icon>
      </md-filled-tonal-icon-button>
      <md-filled-tonal-icon-button
        class="telegram-action"
        title="Telegram"
        aria-label="Telegram"
        @click="API.openLink(TELEGRAM_URL)"
      >
        <md-icon
          ><svg viewBox="0 0 24 24"><path :d="ICONS.telegram" /></svg
        ></md-icon>
      </md-filled-tonal-icon-button>
    </Md3BottomActions>
  </div>
</template>
