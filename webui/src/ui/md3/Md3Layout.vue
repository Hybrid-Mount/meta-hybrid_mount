<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import "./theme.css";
import { ref, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { sysStore } from "../../lib/stores/sysStore";

const { t } = useI18n();

defineProps<{
  navindex: number;
  activepage: Component;
  titles: string[];
}>();

const emit = defineEmits<{
  (e: "update:navindex", value: number): void;
}>();

const rebootOpen = ref(false);

function rebootSystem(): void {
  sysStore.rebootDevice();
  rebootOpen.value = false;
}
</script>

<template>
  <div class="md3-app">
    <header class="md3-topbar">
      <span class="md3-title">{{ t("common.appName") }}</span>
      <button
        class="md3-icon-button"
        :aria-label="t('common.reboot')"
        @click="rebootOpen = true"
      >
        ↻
      </button>
    </header>

    <main class="md3-body">
      <Transition name="fade" mode="out-in">
        <component :is="activepage" v-if="activepage" :key="navindex" />
      </Transition>
    </main>

    <nav class="md3-bottom">
      <button
        v-for="(title, index) in titles"
        :key="title"
        class="md3-nav-item"
        :class="{ active: navindex === index }"
        @click="emit('update:navindex', index)"
      >
        <span class="md3-nav-icon">{{ ["◉", "⚙", "▤", "ℹ"][index] }}</span>
        <span>{{ title }}</span>
      </button>
    </nav>

    <div v-if="rebootOpen" class="md3-dialog-mask" @click.self="rebootOpen = false">
      <div class="md3-dialog">
        <h3>{{ t("common.rebootTitle") }}</h3>
        <p>{{ t("common.rebootConfirm") }}</p>
        <div class="md3-dialog-actions">
          <button class="md3-button" @click="rebootOpen = false">
            {{ t("common.cancel") }}
          </button>
          <button class="md3-button md3-button-primary" @click="rebootSystem">
            {{ t("common.reboot") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
