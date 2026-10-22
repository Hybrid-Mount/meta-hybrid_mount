<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { computed, ref } from "vue";
import {
  MiuixButton,
  MiuixText,
  MiuixDialog,
  MiuixRadioButtonPreference,
  MiuixArrowPreference,
} from "miuix-vue";

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
const selectedOption = computed(() =>
  props.options.find((option) => option.value === props.modelValue),
);

function select(option: MiuixSelectOption): void {
  if (option.disabled) return;
  if (option.value !== props.modelValue) emit("update:modelValue", option.value);
  open.value = false;
}
</script>

<template>
  <div class="miuix-select-field" :class="{ compact }">
    <MiuixArrowPreference
      v-if="!compact"
      :disabled="disabled"
      :title="label"
      :summary="summary"
      @click="open = true"
    >
      <template v-if="$slots.start" #start>
        <slot name="start" />
      </template>
      <template #end>
        <MiuixText type="body2" weight="thin">{{
          selectedOption?.label ?? "-"
        }}</MiuixText>
      </template>
    </MiuixArrowPreference>

    <MiuixArrowPreference
      v-else
      :disabled="disabled"
      :title="selectedOption?.label ?? '-'"
      @click="open = true"
    />

    <MiuixDialog v-model="open" :title="label" @close="open = false">
      <div class="select-options">
        <MiuixRadioButtonPreference
          v-for="option in options"
          :key="option.value"
          :model-value="option.value === modelValue"
          :title="option.label"
          :summary="option.description"
          @select="select(option)"
          :disabled="option.disabled"
        />
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

.compact {
  width: min(190px, 100%);
  max-width: 100%;
  flex: 0 1 190px;
}

.select-options {
  max-height: min(56vh, 440px);
  display: flex;
  flex-direction: column;
  gap: 0;
  overflow-y: auto;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  padding-top: 8px;
}
</style>
