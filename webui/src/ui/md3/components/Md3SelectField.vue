<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, ref } from "vue";
import { ICONS } from "../icons";

export interface SelectOption {
  value: string;
  label: string;
  description?: string;
  disabled?: boolean;
}

const props = defineProps<{
  label: string;
  modelValue: string;
  options: SelectOption[];
  compact?: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const dialog = ref<(HTMLElement & { show: () => void; close: () => void }) | null>(null);
const selectedOption = computed(
  () =>
    props.options.find((option) => option.value === props.modelValue) ?? props.options[0],
);

function open(): void {
  dialog.value?.show();
}

function close(): void {
  dialog.value?.close();
}

function select(option: SelectOption): void {
  if (option.disabled) return;
  if (option.value !== props.modelValue) emit("update:modelValue", option.value);
  close();
}
</script>

<template>
  <div class="select-field" :class="{ compact }">
    <button
      type="button"
      class="select-trigger"
      aria-haspopup="dialog"
      :aria-label="label"
      :disabled="disabled"
      @click="open"
    >
      <span class="select-copy">
        <span v-if="!compact" class="select-label">{{ label }}</span>
        <span class="select-value">{{ selectedOption?.label }}</span>
      </span>
      <md-icon class="select-chevron" aria-hidden="true">
        <svg viewBox="0 0 24 24"><path d="M7 10l5 5 5-5z" /></svg>
      </md-icon>
    </button>

    <md-dialog ref="dialog" class="select-dialog">
      <div slot="headline">{{ label }}</div>
      <div slot="content" class="select-options">
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
            <span class="option-label">{{ option.label }}</span>
            <span v-if="option.description" class="option-description">
              {{ option.description }}
            </span>
          </span>
          <md-icon v-if="option.value === modelValue" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path :d="ICONS.check" /></svg>
          </md-icon>
        </button>
      </div>
      <div slot="actions">
        <md-text-button @click="close">{{ $t("common.close") }}</md-text-button>
      </div>
    </md-dialog>
  </div>
</template>

<style scoped>
.select-field {
  width: 100%;
  min-width: 0;
}

.select-trigger {
  width: 100%;
  min-height: 58px;
  padding: 8px 12px 8px 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border: 1px solid var(--md-sys-color-outline);
  border-radius: var(--radius-lg);
  color: var(--md-sys-color-on-surface);
  background: var(--md-sys-color-surface-container-low);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.select-trigger:hover {
  background: var(--md-sys-color-surface-container-high);
}

.select-trigger:focus-visible {
  outline: 2px solid var(--md-sys-color-primary);
  outline-offset: 2px;
}

.select-trigger:disabled {
  cursor: default;
  opacity: 0.5;
}

.select-copy,
.option-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.select-label {
  color: var(--md-sys-color-on-surface-variant);
  font-size: 12px;
  line-height: 16px;
}

.select-value {
  overflow: hidden;
  font-size: 16px;
  line-height: 24px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.compact {
  width: min(190px, 46vw);
  flex: 0 1 190px;
}

.compact .select-trigger {
  min-height: 42px;
  padding: 0 8px 0 12px;
  border-radius: var(--radius-md);
}

.compact .select-value {
  font-size: 14px;
  line-height: 20px;
}

.select-chevron {
  flex: 0 0 auto;
  color: var(--md-sys-color-on-surface-variant);
}

.select-chevron svg,
.select-option md-icon svg {
  width: 24px;
  height: 24px;
  fill: currentColor;
}

.select-dialog {
  width: min(420px, calc(100vw - 32px));
  --md-dialog-container-color: var(--md-sys-color-surface-container-high);
}

.select-options {
  max-height: min(55vh, 440px);
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow-y: auto;
}

.select-option {
  min-height: 52px;
  padding: 10px 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border: 0;
  border-radius: var(--radius-lg);
  color: var(--md-sys-color-on-surface);
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.select-option:hover {
  background: var(--md-sys-color-surface-container-highest);
}

.select-option.selected {
  color: var(--md-sys-color-on-secondary-container);
  background: var(--md-sys-color-secondary-container);
}

.select-option:disabled {
  cursor: default;
  opacity: 0.45;
}

.option-label {
  font-size: 15px;
  font-weight: 600;
}

.option-description {
  margin-top: 2px;
  color: var(--md-sys-color-on-surface-variant);
  font-size: 12px;
}
</style>
