<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import WearLayout from "./WearLayout.vue";
import StatusPage from "../md3/pages/status.vue";
import ConfigPage from "../md3/pages/config.vue";
import ModulesPage from "../md3/pages/modules.vue";
import InfoPage from "../md3/pages/info.vue";

const { t } = useI18n();
const pages = [StatusPage, ConfigPage, ModulesPage, InfoPage];
const titles = computed(() => [
  t("tabs.status"),
  t("tabs.config"),
  t("tabs.modules"),
  t("tabs.info"),
]);
</script>

<template>
  <WearLayout :pages="pages" :titles="titles" />
</template>
