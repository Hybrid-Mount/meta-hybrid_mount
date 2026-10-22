<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import {
  MiuixBasicComponent,
  MiuixButton,
  MiuixCard,
  MiuixDivider,
  MiuixIcon,
  MiuixText,
} from "miuix-vue";
import { Close } from "miuix-vue/icons"

import type { BannerAction, StatusError } from "../../../lib/statusMounts";

defineProps<{
  title: string;
  /** Reason the banner is on screen at all; empty hides it. */
  hint: string;
  errors: StatusError[];
  details: { label: string; value: string }[];
  itemsLabel: string;
  /** Recovery offered under the problems; null leaves the banner read-only. */
  action?: BannerAction | null;
}>();

const emit = defineEmits<{
  (event: "action"): void;
}>();

const cardStyle = {
  "--m-card-color": "var(--m-color-error-container)",
};
</script>

<template>
  <MiuixCard
    v-if="errors.length > 0"
    class="card"
    role="alert"
    aria-live="assertive"
    :style="cardStyle"
  >
    <MiuixBasicComponent
      :title="title"
      :summary="hint || undefined"
      title-color="currentColor"
    >
      <template #start>
        <MiuixIcon :icon="Close" :size="22" color="var(--m-color-error)" />
      </template>
    </MiuixBasicComponent>

    <MiuixBasicComponent
      v-for="(error, index) in errors"
      :key="`${error.code}-${index}`"
      :title="$t(error.code)"
      title-color="currentColor"
    >
      <template #bottom>
        <pre v-if="error.detail" class="error-banner__detail">{{ error.detail }}</pre>
        <div v-if="error.items.length" class="error-banner__items">
          <MiuixText type="footnote2" class="error-banner__items-label">
            {{ itemsLabel }}
          </MiuixText>
          <code v-for="item in error.items" :key="item">{{ item }}</code>
        </div>
      </template>
    </MiuixBasicComponent>

    <!-- MiuixDivider is width:100%, so the inset comes from the wrapper. -->
    <div v-if="details.length" class="error-banner__rule">
      <MiuixDivider color="color-mix(in srgb, var(--m-color-error) 30%, transparent)" />
    </div>

    <!-- Compact metadata: BasicComponent rows would cost 56px each, and these
         are secondary to the problems above. -->
    <dl v-if="details.length" class="error-banner__rows">
      <template v-for="row in details" :key="row.label">
        <dt>
          <MiuixText type="footnote1">{{ row.label }}</MiuixText>
        </dt>
        <dd>
          <MiuixText type="footnote1">{{ row.value }}</MiuixText>
        </dd>
      </template>
    </dl>

    <div v-if="action" class="error-banner__action">
      <MiuixButton :disabled="action.busy" @click="emit('action')">
        {{ action.label }}
      </MiuixButton>
    </div>
  </MiuixCard>
</template>

<style scoped>
.error-banner__detail {
  margin: 0;
  padding: 6px 8px;
  border-radius: var(--m-radius-xs, 8px);
  background: color-mix(in srgb, var(--m-color-error) 14%, transparent);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.error-banner__items {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
}

.error-banner__items-label {
  opacity: 0.85;
}

.error-banner__items code {
  padding: 1px 8px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--m-color-error) 18%, transparent);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.error-banner__rule {
  padding: 4px 16px 8px;
}

.error-banner__rows {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 4px 12px;
  margin: 0;
  padding: 0 16px 8px;
}

.error-banner__rows dt {
  opacity: 0.8;
}

.error-banner__rows dd {
  margin: 0;
  overflow-wrap: anywhere;
}

.error-banner__action {
  display: flex;
  justify-content: flex-end;
  padding: 0 16px 16px;
}
</style>
