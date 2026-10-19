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
/* Sits inside the expanded module card. The card's rows use a 16px inside
 * margin, so this block matches it instead of adding its own outer spacing. */
.runtime-controls {
  padding: 12px 16px;
  border-bottom: 1px solid var(--m-color-divider-line, rgba(0, 0, 0, 0.06));
}

.runtime-heading {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.runtime-heading h3 {
  margin: 0;
  color: var(--m-color-on-surface);
  font-size: var(--m-text-headline1-size, 17px);
  font-weight: 500;
  line-height: 22px;
}

.runtime-state {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--m-color-on-surface-variant-summary);
  font-size: var(--m-text-footnote1-size, 13px);
  line-height: 18px;
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
  font-size: var(--m-text-body2-size, 14px);
  line-height: 20px;
  overflow-wrap: anywhere;
}

.runtime-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 12px;
}

.runtime-secondary:not(:disabled) {
  color: var(--m-color-on-surface-variant-summary);
  background: transparent;
}

.runtime-error {
  color: var(--m-color-error);
}
</style>
