<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
defineProps<{ label: string; icon: string; wear?: boolean }>();
</script>

<template>
  <button v-if="wear" type="button" class="wear-button wear-action">{{ label }}</button>
  <md-filled-tonal-icon-button v-else :title="label" :aria-label="label">
    <md-icon
      ><svg viewBox="0 0 24 24"><path :d="icon" /></svg
    ></md-icon>
  </md-filled-tonal-icon-button>
</template>
