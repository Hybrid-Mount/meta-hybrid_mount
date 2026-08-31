<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import { h, computed, ref, type FunctionalComponent } from "vue";
import { MiuixButton, MiuixDialog, MiuixRadioButtonPreference } from "miuix-vue";

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

interface IconPath {
  d: string;
  /**
   * Per-path opacity for multi-color icons.
   */
  opacity?: number;
  fillRule?: "evenodd" | "nonzero";
}

interface IconSpec {
  /**
   * Intrinsic width/height in dp (→ px).
   */
  width: number;
  height: number;
  /**
   * ViewBox dimensions.
   */
  vw: number;
  vh: number;
  paths: IconPath[];
}

function makeIcon(name: string, spec: IconSpec): FunctionalComponent {
  const comp: FunctionalComponent = () =>
    h(
      "svg",
      {
        xmlns: "http://www.w3.org/2000/svg",
        width: spec.width,
        height: spec.height,
        viewBox: `0 0 ${spec.vw} ${spec.vh}`,
        fill: "currentColor",
      },
      spec.paths.map((p) =>
        h("path", {
          d: p.d,
          "fill-rule": p.fillRule ?? "evenodd",
          "clip-rule": p.fillRule ?? "evenodd",
          ...(p.opacity != null ? { "fill-opacity": p.opacity } : {}),
        }),
      ),
    );
  comp.displayName = name;
  return comp;
}

const IconArrowRight = makeIcon("ArrowRight", {
  width: 10,
  height: 16,
  vw: 10,
  vh: 16,
  paths: [
    {
      d: "M1.65 1.469 C1.929 1.19 2.381 1.19 2.66 1.469 L8.721 7.53 C9 7.809 9 8.261 8.721 8.54 L2.66 14.601 C2.381 14.88 1.929 14.88 1.65 14.601 C1.371 14.322 1.371 13.87 1.65 13.591 L7.205 8.035 L1.65 2.479 C1.371 2.2 1.371 1.748 1.65 1.469 Z",
    },
  ],
});

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
      <span><IconArrowRight /></span>
    </button>

    <MiuixDialog v-model="open" :title="label" @close="open = false">
      <div class="select-options">
        <MiuixRadioButtonPreference
          v-for="option in options"
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
