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
      <md-icon aria-hidden="true">
        <svg viewBox="0 0 24 24">
          <path
            d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"
          />
        </svg>
      </md-icon>
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
