<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { uiStore } from "./lib/stores/uiStore";

const MiuixApp = defineAsyncComponent(() => import("./ui/miuix/App.vue"));
const Md3App = defineAsyncComponent(() => import("./ui/md3/App.vue"));
const CurrentApp = computed(() => (uiStore.uiStyle === "md3" ? Md3App : MiuixApp));
</script>

<template>
  <component :is="CurrentApp" />
</template>
