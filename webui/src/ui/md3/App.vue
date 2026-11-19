<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { uiStore } from "../../lib/stores/uiStore";
import Md3Layout from "./Md3Layout.vue";
import StatusPage from "./pages/status.vue";
import ConfigPage from "./pages/config.vue";
import ModulesPage from "./pages/modules.vue";
import InfoPage from "./pages/info.vue";
import { useI18n } from "vue-i18n";

defineProps<{ wear?: boolean }>();
const WearLayout = defineAsyncComponent(() => import("../wear/WearLayout.vue"));
const { t } = useI18n();
const pages = [StatusPage, ConfigPage, ModulesPage, InfoPage];
const navindex = computed({
  get: () => uiStore.navindex,
  set: (value: number) => uiStore.setNavindex(value),
});
const titles = computed(() => [
  t("tabs.status"),
  t("tabs.config"),
  t("tabs.modules"),
  t("tabs.info"),
]);
</script>

<template>
  <WearLayout v-if="wear" :pages="pages" :titles="titles" />
  <Md3Layout v-else v-model:navindex="navindex" :pages="pages" :titles="titles" />
</template>
