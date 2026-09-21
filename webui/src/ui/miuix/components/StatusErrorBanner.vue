<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
import type { StatusError } from "../../../lib/statusMounts";

defineProps<{
  title: string;
  /** Reason the banner is on screen at all; empty hides it. */
  hint: string;
  errors: StatusError[];
  details: { label: string; value: string }[];
  itemsLabel: string;
}>();
</script>

<template>
  <section
    v-if="errors.length > 0"
    class="error-banner"
    role="alert"
    aria-live="assertive"
  >
    <header class="error-banner__head">
      <span class="error-banner__symbol" aria-hidden="true">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 -960 960 960"
          fill="currentColor"
        >
          <path
            d="m480-438 129 129q9 9 21 9t21-9q9-9 9-21t-9-21L522-480l129-129q9-9 9-21t-9-21q-9-9-21-9t-21 9L480-522 351-651q-9-9-21-9t-21 9q-9 9-9 21t9 21l129 129-129 129q-9 9-9 21t9 21q9 9 21 9t21-9l129-129Zm0 358q-82 0-155-31.5t-127.5-86Q143-252 111.5-325T80-480q0-83 31.5-156t86-127Q252-817 325-848.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 82-31.5 155T763-197.5q-54 54.5-127 86T480-80Zm0-60q142 0 241-99.5T820-480q0-142-99-241t-241-99q-141 0-240.5 99T140-480q0 141 99.5 240.5T480-140Zm0-340Z"
          />
        </svg>
      </span>
      <strong>{{ title }}</strong>
    </header>
    <p v-if="hint" class="error-banner__hint">{{ hint }}</p>
    <ul class="error-banner__list">
      <li v-for="(error, index) in errors" :key="`${error.code}-${index}`">
        <span class="error-banner__message">{{ $t(error.code) }}</span>
        <pre v-if="error.detail" class="error-banner__detail">{{ error.detail }}</pre>
        <div v-if="error.items.length" class="error-banner__items">
          <span class="error-banner__items-label">{{ itemsLabel }}</span>
          <code v-for="item in error.items" :key="item">{{ item }}</code>
        </div>
      </li>
    </ul>
    <dl v-if="details.length" class="error-banner__rows">
      <template v-for="row in details" :key="row.label">
        <dt>{{ row.label }}</dt>
        <dd>{{ row.value }}</dd>
      </template>
    </dl>
  </section>
</template>

<style scoped>
.error-banner {
  margin: 0 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  border-radius: var(--m-radius-md, 16px);
  border: 2px solid var(--m-color-error);
  color: var(--m-color-on-error-container);
  background: var(--m-color-error-container);
}

.error-banner__head {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 17px;
}

.error-banner__symbol {
  flex-shrink: 0;
  display: inline-flex;
  color: var(--m-color-error);
}

.error-banner__symbol svg {
  width: 22px;
  height: 22px;
}

.error-banner__hint {
  margin: 0;
  font-size: 13px;
  line-height: 1.5;
  opacity: 0.9;
}

.error-banner__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.error-banner__list li {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-radius: var(--m-radius-sm, 10px);
  background: color-mix(in srgb, var(--m-color-error) 12%, transparent);
}

.error-banner__message {
  font-weight: 600;
  font-size: 14px;
}

.error-banner__detail {
  margin: 0;
  padding: 8px 10px;
  border-radius: var(--m-radius-xs, 6px);
  background: color-mix(in srgb, var(--m-color-error) 18%, transparent);
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
}

.error-banner__items-label {
  font-size: 12px;
  opacity: 0.85;
}

.error-banner__items code {
  padding: 2px 8px;
  border-radius: 9999px;
  background: color-mix(in srgb, var(--m-color-error) 22%, transparent);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.error-banner__rows {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: 4px 12px;
  margin: 0;
  font-size: 13px;
}

.error-banner__rows dt {
  opacity: 0.85;
}

.error-banner__rows dd {
  margin: 0;
  overflow-wrap: anywhere;
}
</style>
