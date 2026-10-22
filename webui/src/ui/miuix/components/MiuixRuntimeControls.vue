<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { MiuixButton, MiuixProgressIndicator, MiuixSurface, MiuixText } from "miuix-vue";
import { useRuntimeModule } from "../../../lib/useRuntimeModule";

const props = defineProps<{ moduleId: string }>();
const { t } = useI18n();
const { module, reason, disabled, actionError, working, run } = useRuntimeModule(
  () => props.moduleId,
);

const MUTED = "var(--m-color-on-surface-variant-summary)";
</script>

<template>
  <MiuixSurface
    class="runtime-controls"
    :radius="16"
    color="var(--m-color-surface-container-highest)"
    :aria-label="t('runtime.title')"
    :aria-busy="working"
  >
    <div class="runtime-heading">
      <MiuixText type="headline1" weight="medium">{{ t("runtime.title") }}</MiuixText>
      <span
        v-if="module"
        class="runtime-state"
        :class="{ 'runtime-state--active': module.active }"
        role="status"
      >
        <span class="runtime-state-dot" aria-hidden="true" />
        <MiuixText
          type="footnote1"
          :color="module.active ? 'var(--m-color-primary)' : MUTED"
        >
          {{ t(module.active ? "runtime.active" : "runtime.inactive") }}
        </MiuixText>
      </span>
    </div>

    <MiuixText as="p" type="body2" :color="MUTED" class="runtime-copy">
      {{ t("runtime.description") }}
    </MiuixText>

    <div class="runtime-buttons">
      <MiuixButton
        type="primary"
        :disabled="disabled"
        :title="reason ?? undefined"
        @click="run(module?.active ? 'reload' : 'load')"
      >
        {{ t(module?.active ? "runtime.reload" : "runtime.load") }}
      </MiuixButton>
      <MiuixButton
        v-if="module?.active"
        :disabled="disabled"
        :title="reason ?? undefined"
        @click="run('unload')"
      >
        {{ t("runtime.unload") }}
      </MiuixButton>
    </div>

    <MiuixText
      v-if="working"
      as="p"
      type="footnote1"
      :color="MUTED"
      class="runtime-copy runtime-busy"
      role="status"
    >
      <MiuixProgressIndicator type="infinite" :size="14" color="var(--m-color-primary)" />
      {{ t("runtime.busy") }}
    </MiuixText>
    <MiuixText v-if="reason" as="p" type="footnote1" :color="MUTED" class="runtime-copy">
      {{ reason }}
    </MiuixText>
    <MiuixText
      v-if="actionError"
      as="p"
      type="body2"
      color="var(--m-color-error)"
      class="runtime-copy"
      role="alert"
    >
      {{ actionError }}
    </MiuixText>
  </MiuixSurface>
</template>

<style scoped>
/* Nested panel inside the expanded module card: the card's rows sit on a 16px
 * inside margin, so the panel keeps the same inset on both sides. */
.runtime-controls {
  margin: 12px 16px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.runtime-heading {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.runtime-state {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  text-align: right;
}

.runtime-state-dot {
  width: 5px;
  height: 5px;
  flex: 0 0 auto;
  border-radius: 50%;
  background: var(--m-color-on-surface-variant-summary);
}

.runtime-state--active .runtime-state-dot {
  background: var(--m-color-primary);
}

.runtime-copy {
  margin: 0;
  overflow-wrap: anywhere;
}

.runtime-busy {
  display: flex;
  align-items: center;
  gap: 8px;
}

.runtime-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 6px;
}
</style>
