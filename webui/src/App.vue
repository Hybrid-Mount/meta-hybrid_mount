<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { defineAsyncComponent } from "vue";
import { uiStore } from "./lib/stores/uiStore";

const MiuixApp = defineAsyncComponent(() => import("./ui/miuix/App.vue"));
const Md3App = defineAsyncComponent(() => import("./ui/md3/App.vue"));
</script>

<template>
  <Md3App v-if="uiStore.uiStyle === 'md3'" />
  <MiuixApp v-else />
</template>
