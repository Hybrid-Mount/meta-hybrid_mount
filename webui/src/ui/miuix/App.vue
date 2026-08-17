<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, onBeforeUnmount } from "vue";
import { useI18n } from "vue-i18n";
import { showSnackbar } from "miuix-vue";
import { uiStore } from "../../lib/stores/uiStore";
import MiuixLayout from "./MiuixLayout.vue";
import StatusPage from "./pages/status.vue";
import ConfigPage from "./pages/config.vue";
import ModulesPage from "./pages/modules.vue";
import InfoPage from "./pages/info.vue";

const { t } = useI18n();
const pages = [StatusPage, ConfigPage, ModulesPage, InfoPage];
const navindex = computed({
  get: () => uiStore.navindex,
  set: (value: number) => uiStore.setNavindex(value),
});
const activepage = computed(() => pages[navindex.value]);
const titles = computed(() => [
  t("tabs.status"),
  t("tabs.config"),
  t("tabs.modules"),
  t("tabs.info"),
]);

uiStore.setToastHandler((text) => showSnackbar({ message: text }));
onBeforeUnmount(() => uiStore.setToastHandler());
</script>

<template>
  <MiuixLayout v-model:navindex="navindex" :activepage="activepage" :titles="titles" />
</template>
