<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { MiuixCard, MiuixSmallTitle, MiuixBasicComponent } from "miuix-vue";
import DynamicLogo from "../../../components/DynamicLogo.vue";
import { REPOSITORY_URL, TELEGRAM_URL } from "../../../lib/constants";
import {
  loadGitHubContributors,
  type GitHubContributor,
} from "../../../lib/contributors";
import { sysStore } from "../../../lib/stores/sysStore";
import { API } from "../../../lib/api";

const { t } = useI18n();
const socialIcons = {
  github:
    "M12 0C5.37 0 0 5.37 0 12c0 5.3 3.44 9.8 8.21 11.39.6.11.79-.26.79-.58v-2.23c-3.34.72-4.03-1.42-4.03-1.42-.55-1.38-1.33-1.75-1.33-1.75-1.09-.75.08-.73.08-.73 1.21.09 1.84 1.24 1.84 1.24 1.07 1.83 2.81 1.3 3.49 1 .11-.78.42-1.31.76-1.61-2.66-.3-5.47-1.33-5.47-5.93 0-1.31.47-2.38 1.24-3.22-.12-.3-.54-1.52.12-3.18 0 0 1.01-.32 3.3 1.23A11.5 11.5 0 0 1 12 6.4c1.02 0 2.05.14 3.01.4 2.29-1.55 3.3-1.23 3.3-1.23.65 1.66.24 2.88.12 3.18.77.84 1.24 1.91 1.24 3.22 0 4.61-2.81 5.62-5.48 5.92.43.37.82 1.1.82 2.22v3.3c0 .32.19.69.8.57A12 12 0 0 0 12 0Z",
  telegram:
    "M11.94 0A12 12 0 1 0 12 24a12 12 0 0 0-.06-24Zm4.97 7.22c.1 0 .32.03.46.14.12.1.16.23.17.33.02.09.04.3.02.47-.18 1.9-.96 6.5-1.36 8.63-.17.9-.5 1.2-.82 1.23-.7.06-1.23-.46-1.9-.9-1.06-.7-1.65-1.13-2.68-1.8-1.18-.78-.42-1.21.26-1.91.17-.18 3.25-2.98 3.3-3.23.01-.03.02-.15-.05-.21-.07-.06-.18-.04-.25-.02-.11.02-1.8 1.14-5.06 3.34-.48.33-.91.49-1.3.48-.43-.01-1.25-.24-1.87-.44-.75-.24-1.35-.37-1.3-.79.03-.22.33-.44.9-.66 3.5-1.53 5.83-2.53 7-3.02 3.33-1.38 4.02-1.62 4.48-1.63Z",
};

const contributors = ref<GitHubContributor[]>([]);
const loadingContributors = ref(false);
const loadFailed = ref(false);

async function loadContributors(): Promise<void> {
  if (contributors.value.length) return;
  loadingContributors.value = true;
  loadFailed.value = false;
  try {
    contributors.value = await loadGitHubContributors();
  } catch {
    loadFailed.value = true;
  } finally {
    loadingContributors.value = false;
  }
}

onMounted(async () => {
  await sysStore.ensureStatusLoaded();
  await loadContributors();
});
</script>

<template>
  <div class="page">
    <MiuixCard class="card">
      <div class="about-hero">
        <div class="about-logo"><DynamicLogo /></div>
        <strong>{{ t("content.welcome") }}</strong>
      </div>
      <div class="about-meta">
        <div class="about-meta-item">
          <span>{{ t("info.version") }}</span>
          <strong>{{ sysStore.version }}</strong>
        </div>
        <div class="about-meta-item">
          <span>{{ t("info.license") }}</span>
          <strong>GPL-3.0-only · Apache-2.0</strong>
        </div>
      </div>
      <div class="social-links">
        <button
          type="button"
          class="social-link repository-link"
          :aria-label="t('info.projectLink')"
          @click="API.openLink(REPOSITORY_URL)"
        >
          <span class="social-visual">
            <span class="social-icon">
              <svg viewBox="0 0 24 24"><path :d="socialIcons.github" /></svg>
            </span>
            <svg class="social-arrow" viewBox="0 0 24 24" aria-hidden="true">
              <path d="m9 18 6-6-6-6" />
            </svg>
          </span>
          <span class="social-copy">
            <strong>{{ t("info.projectLink") }}</strong>
            <span>github.com/Hybrid-Mount</span>
          </span>
        </button>

        <button
          type="button"
          class="social-link telegram-link"
          aria-label="Telegram"
          @click="API.openLink(TELEGRAM_URL)"
        >
          <span class="social-visual">
            <span class="social-icon">
              <svg viewBox="0 0 24 24"><path :d="socialIcons.telegram" /></svg>
            </span>
            <svg class="social-arrow" viewBox="0 0 24 24" aria-hidden="true">
              <path d="m9 18 6-6-6-6" />
            </svg>
          </span>
          <span class="social-copy">
            <strong>Telegram</strong>
            <span>@hybridmountchat</span>
          </span>
        </button>
      </div>
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
        :title="contributor.name || contributor.login"
        :summary="`@${contributor.login} · ${contributor.bio || t('info.noBio')}`"
        @click="API.openLink(contributor.html_url)"
      />
    </MiuixCard>
  </div>
</template>

<style scoped>
.about-hero {
  padding: 20px 20px 10px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  text-align: center;
}

.about-logo {
  width: 88px;
  height: 88px;
}

.about-hero strong {
  font-size: 24px;
}

.about-hero span {
  color: var(--m-color-on-surface-variant, rgba(0, 0, 0, 0.6));
  font-size: 14px;
}

.about-meta {
  padding: 8px 16px 4px;
  display: grid;
  grid-template-columns: minmax(0, 0.7fr) minmax(0, 1.3fr);
  gap: 10px;
}

.about-meta-item {
  min-width: 0;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  border-radius: 16px;
  background: var(--m-color-surface-container-low, rgba(0, 0, 0, 0.035));
  text-align: left;
}

.about-meta-item span {
  color: var(--m-color-on-surface-variant, rgba(0, 0, 0, 0.6));
  font-size: 12px;
}

.about-meta-item strong {
  overflow: hidden;
  font-size: 14px;
  line-height: 20px;
  text-overflow: ellipsis;
}

.social-links {
  width: 100%;
  padding: 10px 16px 12px;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  box-sizing: border-box;
}

.social-link {
  position: relative;
  min-width: 0;
  min-height: 108px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 12px;
  overflow: hidden;
  border: 0;
  border-radius: 22px;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition:
    transform 160ms ease,
    filter 160ms ease;
}

.social-link::after {
  content: "";
  position: absolute;
  right: -28px;
  bottom: -36px;
  width: 96px;
  height: 96px;
  border-radius: 50%;
  background: currentColor;
  opacity: 0.055;
  pointer-events: none;
}

.repository-link {
  color: var(--m-color-on-secondary-container, #1d192b);
  background: var(--m-color-secondary-container, #e8def8);
}

.telegram-link {
  color: var(--m-color-on-primary-container, #21005d);
  background: var(--m-color-primary-container, #eaddff);
}

.social-link:hover {
  filter: brightness(0.98);
  transform: translateY(-2px);
}

.social-link:active {
  transform: scale(0.98);
}

.social-link:focus-visible {
  outline: 3px solid var(--m-color-primary, #6750a4);
  outline-offset: 2px;
}

.social-visual {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
}

.social-icon {
  width: 40px;
  height: 40px;
  display: grid;
  place-items: center;
  border-radius: 15px;
  color: var(--m-color-on-primary, #fff);
  background: var(--m-color-primary, #6750a4);
  box-shadow: 0 6px 16px rgba(69, 49, 112, 0.2);
}

.repository-link .social-icon {
  color: var(--m-color-surface, #fff);
  background: var(--m-color-on-surface, #1d1b20);
}

.social-icon svg {
  width: 22px;
  height: 22px;
  fill: currentColor;
}

.social-arrow {
  width: 23px;
  height: 23px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
  opacity: 0.66;
}

.social-copy {
  position: relative;
  z-index: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.social-copy strong {
  font-size: 17px;
  line-height: 22px;
}

.social-copy span {
  overflow: hidden;
  font-size: 12px;
  line-height: 16px;
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0.72;
}

@media (prefers-reduced-motion: reduce) {
  .social-link {
    transition: none;
  }
}

@media (max-width: 560px) {
  .about-meta,
  .social-links {
    grid-template-columns: 1fr;
  }
}
</style>
