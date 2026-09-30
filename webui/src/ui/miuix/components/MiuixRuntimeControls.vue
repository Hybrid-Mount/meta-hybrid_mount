<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { MiuixButton } from "miuix-vue";
import { useRuntimeModule } from "../../../lib/useRuntimeModule";

const props = defineProps<{ moduleId: string }>();
const { t } = useI18n();
const { module, reason, disabled, actionError, working, run } = useRuntimeModule(
  () => props.moduleId,
);
</script>

<template>
  <section class="runtime-controls" :aria-label="t('runtime.title')" :aria-busy="working">
    <div class="runtime-heading">
      <h3>{{ t("runtime.title") }}</h3>
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
    <p class="runtime-description">{{ t("runtime.description") }}</p>
    <div class="runtime-buttons">
      <MiuixButton
        class="runtime-primary"
        :disabled="disabled"
        :title="reason ?? undefined"
        @click="run(module?.active ? 'reload' : 'load')"
      >
        {{ t(module?.active ? "runtime.reload" : "runtime.load") }}
      </MiuixButton>
      <MiuixButton
        v-if="module?.active"
        class="runtime-secondary"
        :disabled="disabled"
        :title="reason ?? undefined"
        @click="run('unload')"
      >
        {{ t("runtime.unload") }}
      </MiuixButton>
    </div>
    <p v-if="working" class="runtime-message" role="status">{{ t("runtime.busy") }}</p>
    <p v-if="reason" class="runtime-message">{{ reason }}</p>
    <p v-if="actionError" class="runtime-error" role="alert">{{ actionError }}</p>
  </section>
</template>

<style scoped>
.runtime-controls {
  padding: 16px 0 18px;
  margin-bottom: 8px;
  border-bottom: 1px solid var(--m-color-divider-line, rgba(0, 0, 0, 0.06));
}

.runtime-heading {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.runtime-heading h3 {
  margin: 0;
  color: var(--m-color-on-surface);
  font-size: 16px;
  font-weight: 500;
  line-height: 22px;
}

.runtime-state {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--m-color-on-surface-variant-summary);
  font-size: 13px;
  line-height: 20px;
  text-align: right;
}

.runtime-state.active {
  color: var(--m-color-on-surface-secondary);
}

.runtime-state-dot {
  width: 5px;
  height: 5px;
  flex: 0 0 auto;
  border-radius: 50%;
  background: currentColor;
}

.runtime-state.active .runtime-state-dot {
  background: var(--m-color-primary);
}

.runtime-description,
.runtime-message,
.runtime-error {
  margin: 4px 0 0;
  color: var(--m-color-on-surface-variant-summary);
  font-size: 13px;
  line-height: 20px;
  overflow-wrap: anywhere;
}

.runtime-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 14px;
}

.runtime-buttons :deep(.m-button) {
  min-height: 44px;
  border-radius: 14px;
  padding-inline: 20px;
  font-size: 14px;
  font-weight: 500;
  line-height: 20px;
}

.runtime-primary:not(:disabled) {
  color: var(--m-color-on-secondary-variant);
}

.runtime-secondary:not(:disabled) {
  color: var(--m-color-on-surface-variant-summary);
  background: transparent;
}

.runtime-message,
.runtime-error {
  margin-top: 10px;
}

.runtime-error {
  color: var(--m-color-error);
}
</style>
