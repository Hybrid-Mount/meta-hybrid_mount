<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { defineAsyncComponent } from "vue";
import { uiStore } from "./lib/stores/uiStore";

defineProps<{ wear: boolean }>();
const MiuixApp = defineAsyncComponent(() => import("./ui/miuix/App.vue"));
const Md3App = defineAsyncComponent(() => import("./ui/md3/App.vue"));
</script>

<template>
  <Md3App v-if="wear || uiStore.uiStyle === 'md3'" :wear="wear" />
  <MiuixApp v-else />
</template>
