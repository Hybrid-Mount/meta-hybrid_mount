<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, ref } from "vue";
import { MiuixButton, MiuixDialog, MiuixIcon } from "miuix-vue";
import { IconCheck } from "miuix-vue";
import { ExpandMore } from "miuix-vue/icons";

export interface MiuixSelectOption {
  value: string;
  label: string;
  description?: string;
  disabled?: boolean;
}

const props = withDefaults(
  defineProps<{
    label: string;
    modelValue: string;
    options: MiuixSelectOption[];
    summary?: string;
    compact?: boolean;
    disabled?: boolean;
  }>(),
  {
    summary: "",
    compact: false,
    disabled: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const open = ref(false);
const selectedOption = computed(
  () =>
    props.options.find((option) => option.value === props.modelValue) ?? props.options[0],
);

function select(option: MiuixSelectOption): void {
  if (option.disabled) return;
  if (option.value !== props.modelValue) emit("update:modelValue", option.value);
  open.value = false;
}
</script>

<template>
  <div class="miuix-select-field" :class="{ compact }">
    <button
      type="button"
      class="miuix-select-trigger"
      :disabled="disabled"
      aria-haspopup="dialog"
      :aria-label="label"
      @click="open = true"
    >
      <span v-if="!compact" class="trigger-copy">
        <strong>{{ label }}</strong>
        <span v-if="summary">{{ summary }}</span>
      </span>
      <span class="trigger-value">{{ selectedOption?.label ?? "-" }}</span>
      <MiuixIcon :icon="ExpandMore" :size="18" aria-hidden="true" />
    </button>

    <MiuixDialog v-model="open" :title="label" @close="open = false">
      <div class="select-options">
        <button
          v-for="option in options"
          :key="option.value"
          type="button"
          class="select-option"
          :class="{ selected: option.value === modelValue }"
          :disabled="option.disabled"
          :aria-pressed="option.value === modelValue"
          @click="select(option)"
        >
          <span class="option-copy">
            <strong>{{ option.label }}</strong>
            <span v-if="option.description">{{ option.description }}</span>
          </span>
          <IconCheck v-if="option.value === modelValue" class="option-check" />
        </button>
      </div>
      <div class="dialog-actions">
        <MiuixButton @click="open = false">{{ $t("common.close") }}</MiuixButton>
      </div>
    </MiuixDialog>
  </div>
</template>

<style scoped>
.miuix-select-field {
  width: 100%;
  min-width: 0;
}

.miuix-select-trigger {
  width: 100%;
  min-height: 72px;
  padding: 12px 16px;
  display: flex;
  align-items: center;
  gap: 10px;
  border: 0;
  border-radius: 16px;
  color: var(--m-color-on-surface, #1d1b20);
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.miuix-select-trigger:hover {
  background: var(--m-color-surface-container-high, rgba(0, 0, 0, 0.06));
}

.miuix-select-trigger:focus-visible {
  outline: 2px solid var(--m-color-primary, #6750a4);
  outline-offset: -2px;
}

.miuix-select-trigger:disabled {
  cursor: default;
  opacity: 0.5;
}

.trigger-copy,
.option-copy {
  min-width: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
}

.trigger-copy strong {
  font-size: 17px;
  line-height: 22px;
}

.trigger-copy span,
.option-copy span {
  margin-top: 2px;
  color: var(--m-color-on-surface-variant-summary, rgba(0, 0, 0, 0.6));
  font-size: 13px;
  line-height: 18px;
}

.trigger-value {
  min-width: 0;
  overflow: hidden;
  color: var(--m-color-on-surface-variant-actions, rgba(0, 0, 0, 0.62));
  font-size: 14px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.compact {
  width: min(190px, 100%);
  max-width: 100%;
  flex: 0 1 190px;
}

.compact .miuix-select-trigger {
  min-height: 40px;
  padding: 0 10px 0 12px;
  border-radius: 13px;
  background: var(--m-color-surface-container-high, rgba(0, 0, 0, 0.06));
}

.compact .trigger-value {
  flex: 1;
  color: var(--m-color-on-surface, #1d1b20);
  text-align: left;
}

.select-options {
  max-height: min(56vh, 440px);
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow-y: auto;
}

.select-option {
  width: 100%;
  min-height: 52px;
  padding: 10px 14px;
  display: flex;
  align-items: center;
  gap: 12px;
  border: 0;
  border-radius: 16px;
  color: var(--m-color-on-surface, #1d1b20);
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.select-option:hover {
  background: var(--m-color-surface-container-high, rgba(0, 0, 0, 0.06));
}

.select-option.selected {
  color: var(--m-color-on-primary-container, #21005d);
  background: var(--m-color-primary-container, #eaddff);
}

.select-option:disabled {
  cursor: default;
  opacity: 0.45;
}

.option-copy strong {
  font-size: 15px;
  line-height: 20px;
}

.option-check {
  width: 20px;
  height: 20px;
  flex: 0 0 20px;
  color: currentColor;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  padding-top: 12px;
}

@media (max-width: 420px) {
  .miuix-select-field:not(.compact) .miuix-select-trigger {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 4px 8px;
  }

  .miuix-select-field:not(.compact) .trigger-copy {
    grid-column: 1;
    grid-row: 1;
  }

  .miuix-select-field:not(.compact) .trigger-value {
    grid-column: 1;
    grid-row: 2;
    justify-self: start;
  }

  .miuix-select-field:not(.compact) .miuix-select-trigger > :last-child {
    grid-column: 2;
    grid-row: 1 / span 2;
  }
}
</style>
