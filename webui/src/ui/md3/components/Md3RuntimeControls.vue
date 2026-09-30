<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useRuntimeModule } from "../../../lib/useRuntimeModule";
import { ICONS } from "../icons";

const props = defineProps<{ moduleId: string }>();
const { t } = useI18n();
const { module, reason, disabled, actionError, working, run } = useRuntimeModule(
  () => props.moduleId,
);
</script>

<template>
  <section class="runtime-controls" :aria-label="t('runtime.title')" :aria-busy="working">
    <div class="runtime-heading">
      <h3 class="section-label">{{ t("runtime.title") }}</h3>
      <span
        v-if="module"
        class="runtime-state"
        :class="{ active: module.active }"
        role="status"
      >
        <span class="runtime-state-dot" aria-hidden="true" />
        {{ t(module.active ? "runtime.active" : "runtime.inactive") }}
      </span>
    </div>
    <p class="runtime-caption">{{ t("runtime.description") }}</p>
    <div class="runtime-actions">
      <template v-if="module?.active">
        <button
          type="button"
          class="runtime-action runtime-primary"
          :disabled="disabled"
          @click="run('reload')"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><path :d="ICONS.refresh" /></svg>
          {{ t("runtime.reload") }}
        </button>
        <button
          type="button"
          class="runtime-action"
          :disabled="disabled"
          @click="run('unload')"
        >
          {{ t("runtime.unload") }}
        </button>
      </template>
      <button
        v-else
        type="button"
        class="runtime-action runtime-primary"
        :disabled="disabled"
        @click="run('load')"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path :d="ICONS.power" /></svg>
        {{ t("runtime.load") }}
      </button>
    </div>
    <p v-if="working" class="runtime-feedback" role="status">
      {{ t("runtime.busy") }}
    </p>
    <p v-if="reason" class="runtime-feedback">{{ reason }}</p>
    <p v-if="actionError" class="runtime-feedback runtime-error" role="alert">
      {{ actionError }}
    </p>
  </section>
</template>

<style scoped>
.runtime-controls {
  padding: 16px 0;
  margin-bottom: 16px;
  border-bottom: 1px solid var(--md-sys-color-outline-variant);
}

.runtime-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px 16px;
}

.runtime-heading .section-label {
  margin: 0;
}

.runtime-state {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--md-sys-color-on-surface-variant);
  font-size: 11px;
  font-weight: 500;
  line-height: 18px;
}

.runtime-state.active {
  color: var(--md-sys-color-primary);
}

.runtime-state-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: currentColor;
}

.runtime-caption,
.runtime-feedback {
  margin: 6px 0 0;
  color: var(--md-sys-color-on-surface-variant);
  font-size: 12px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}

.runtime-actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.runtime-action {
  display: inline-flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-width: 0;
  min-height: 44px;
  padding: 10px 16px;
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--md-sys-color-on-surface-variant);
  font-family: inherit;
  font-size: 12px;
  font-weight: 600;
  line-height: 18px;
  cursor: pointer;
  transition:
    background-color 160ms ease,
    opacity 160ms ease;
}

.runtime-primary {
  border-color: transparent;
  background: var(--md-sys-color-secondary-container);
  color: var(--md-sys-color-on-secondary-container);
}

.runtime-action svg {
  flex: 0 0 auto;
  width: 16px;
  height: 16px;
  fill: currentColor;
}

.runtime-action:hover:not(:disabled) {
  background: color-mix(in srgb, currentColor 8%, transparent);
}

.runtime-primary:hover:not(:disabled) {
  background: color-mix(
    in srgb,
    var(--md-sys-color-on-secondary-container) 8%,
    var(--md-sys-color-secondary-container)
  );
}

.runtime-action:focus-visible {
  outline: 2px solid var(--md-sys-color-primary);
  outline-offset: 2px;
}

.runtime-action:disabled {
  opacity: 0.38;
  cursor: default;
}

.runtime-feedback {
  margin-top: 10px;
}

.runtime-error {
  color: var(--md-sys-color-error);
}

@media (min-width: 600px) {
  .runtime-actions {
    max-width: 320px;
  }
}
</style>
