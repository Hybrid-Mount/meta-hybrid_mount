<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  MiuixCard,
  MiuixSmallTitle,
  MiuixBasicComponent,
  MiuixText,
  MiuixProgressIndicator,
  MiuixArrowPreference,
} from "miuix-vue";
import DynamicLogo from "../../../components/DynamicLogo.vue";
import { REPOSITORY_URL, TELEGRAM_URL } from "../../../lib/constants";
import {
  loadGitHubContributors,
  type GitHubContributor,
} from "../../../lib/contributors";
import { sysStore } from "../../../lib/stores/sysStore";
import { API } from "../../../lib/api";

const { t } = useI18n();

const contributors = ref<GitHubContributor[]>([]);
const loadingContributors = ref(false);
const loadFailed = ref(false);

function statusText(value: boolean | null | undefined): string {
  return value == null ? t("info.unverified") : value ? "✓" : "✗";
}

/** How the kernel provides VFS: a loaded module or the kernel image itself. */
function vfsTypeText(value: string | undefined): string {
  if (value === "lkm") return "LKM";
  if (value === "builtin") return "Built-in";
  return t("info.unverified");
}
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
    <div class="about-hero">
      <DynamicLogo class="about-logo" />
      <h1>{{ t("content.welcome") }}</h1>
      <MiuixText color="var(--m-color-on-background)" type="subtitle">{{
        sysStore.version
      }}</MiuixText>
      <MiuixText color="var(--m-color-on-background)" type="body2"
        >GPL-3.0-only · Apache-2.0</MiuixText
      >
    </div>

    <MiuixCard class="card" style="padding-top: 12px">
      <MiuixArrowPreference
        :title="t('info.projectLink')"
        summary="github.com/Hybrid-Mount"
        @click="API.openLink(REPOSITORY_URL)"
      />
      <MiuixArrowPreference
        title="Telegram"
        summary="@hybridmountchat"
        clickable
        @click="API.openLink(TELEGRAM_URL)"
      />
    </MiuixCard>

    <MiuixSmallTitle :text="t('info.installTitle')" />
    <MiuixCard class="card">
      <MiuixBasicComponent :title="t('info.binary')">
        <template #end>
          <MiuixText>{{ statusText(sysStore.installState?.binary) }}</MiuixText>
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent :title="t('info.overlaySupported')">
        <template #end>
          <MiuixText>
            {{ statusText(sysStore.installState?.overlay_supported) }}
          </MiuixText>
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent :title="t('info.tmpfsSupported')">
        <template #end>
          <MiuixText>{{ statusText(sysStore.installState?.tmpfs_supported) }}</MiuixText>
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent :title="t('info.vfsSupported')">
        <template #end>
          <MiuixText>{{ statusText(sysStore.installState?.vfs_supported) }}</MiuixText>
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent :title="t('info.vfsType')">
        <template #end>
          <MiuixText>{{ vfsTypeText(sysStore.installState?.vfs_type) }}</MiuixText>
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent :title="t('info.nukeSupported')">
        <template #end>
          <MiuixText>
            {{
              sysStore.installState?.nuke_type === "ksud"
                ? "ksud"
                : statusText(sysStore.installState?.nuke_supported)
            }}
          </MiuixText>
        </template>
      </MiuixBasicComponent>
      <MiuixBasicComponent :title="t('status.compatible')">
        <template #end>
          <MiuixText>{{ sysStore.installState?.compatible ? "✓" : "✗" }}</MiuixText>
        </template>
      </MiuixBasicComponent>
    </MiuixCard>

    <MiuixSmallTitle :text="t('info.contributors')" />
    <div v-if="loadingContributors" align="center">
      <MiuixProgressIndicator
        type="infinite"
        color="var(--m-color-on-background-variant)"
        style="padding-top: 12px"
      />
    </div>
    <MiuixCard class="card" v-else>
      <MiuixBasicComponent v-if="loadFailed" :title="t('info.loadFail')" />
      <MiuixBasicComponent
        v-else
        v-for="contributor in contributors"
        :key="contributor.login"
        :title="contributor.name || contributor.login"
        :summary="`@${contributor.login} · ${contributor.bio || t('info.noBio')}`"
        @click="API.openLink(contributor.html_url)"
      >
        <template #start>
          <img
            :src="contributor.avatar_url"
            width="42"
            style="margin: 0 8px; border-radius: 12px"
          />
        </template>
      </MiuixBasicComponent>
    </MiuixCard>
  </div>
</template>

<style scoped>
.about-hero {
  padding: 16px 16px 8px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  text-align: center;
}

.about-logo {
  width: 180px;
  height: 180px;
}

.about-hero span {
  color: var(--m-color-on-surface-variant, rgba(0, 0, 0, 0.6));
  font-size: var(--m-text-body2-size, 14px);
}
</style>
