<!-- SPDX-License-Identifier: Apache-2.0 -->
<script setup lang="ts">
const isDevelopment = import.meta.env.DEV;
</script>

<template>
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 120 120"
    class="dynamic-logo"
    :class="{ 'is-development': isDevelopment }"
    aria-hidden="true"
  >
    <circle cx="60" cy="60" r="50" class="logo-base-track" />
    <circle cx="60" cy="60" r="38" class="logo-base-track" />
    <circle cx="60" cy="60" r="26" class="logo-base-track" />

    <template v-if="isDevelopment">
      <g class="dev-logo-outer-group">
        <path d="M 60 10 A 50 50 0 1 1 10 60" class="logo-arc logo-arc-outer" />
      </g>
      <g class="dev-logo-mid-group">
        <path
          d="M 60 22 A 38 38 0 0 1 60 98"
          class="logo-arc logo-arc-mid logo-arc-error"
        />
      </g>
      <g class="dev-logo-inner-group">
        <path d="M 60 34 A 26 26 0 1 1 47 82.5" class="logo-arc logo-arc-inner" />
      </g>
    </template>

    <template v-else>
      <path d="M60 10 A 50 50 0 0 1 110 60" class="logo-arc logo-arc-outer" />
      <path d="M60 98 A 38 38 0 0 1 60 22" class="logo-arc logo-arc-mid" />
      <path d="M34 60 A 26 26 0 1 1 86 60" class="logo-arc logo-arc-inner" />
    </template>

    <circle cx="60" cy="60" r="10" class="logo-core" />
  </svg>
</template>

<style scoped>
.dynamic-logo {
  width: 100%;
  height: 100%;
  overflow: visible;
}

.logo-base-track {
  fill: none;
  stroke: var(
    --md-sys-color-surface-variant,
    var(--m-color-surface-container-high, rgba(127, 127, 127, 0.28))
  );
  opacity: 0.3;
  stroke-width: 1;
}

.logo-arc {
  fill: none;
  stroke-linecap: round;
  transform-origin: center;
}

.logo-arc-outer {
  stroke: var(--md-sys-color-tertiary, var(--m-color-tertiary, #7d5260));
  stroke-width: 4;
  animation: logo-spin-cw 12s linear infinite;
}

.logo-arc-mid {
  stroke: var(--md-sys-color-secondary, var(--m-color-secondary, #625b71));
  stroke-width: 5;
  animation: logo-spin-ccw 8s ease-in-out infinite;
}

.logo-arc-inner {
  stroke: var(--md-sys-color-primary, var(--m-color-primary, #6750a4));
  stroke-width: 6;
  animation: logo-spin-cw 4s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

.logo-core {
  fill: var(--md-sys-color-primary, var(--m-color-primary, #6750a4));
  transform-origin: center;
  animation: logo-pulse 2s ease-in-out infinite;
  filter: drop-shadow(
    0 0 6px
      var(--md-sys-color-primary-container, var(--m-color-primary-container, #eaddff))
  );
}

.dev-logo-outer-group,
.dev-logo-mid-group,
.dev-logo-inner-group {
  transform-origin: center;
}

.dev-logo-outer-group {
  transform: rotate(-45deg);
}

.dev-logo-mid-group {
  transform: rotate(135deg);
}

.dev-logo-inner-group {
  transform: rotate(270deg);
}

.is-development .logo-base-track,
.is-development .logo-arc {
  stroke-width: 8px;
}

.is-development .logo-base-track {
  opacity: 0.1;
}

.is-development .logo-arc-error {
  stroke: var(--md-sys-color-error, var(--m-color-error, #ba1a1a));
  stroke-dasharray: 10 14;
}

.is-development .logo-arc-inner {
  stroke: var(--md-sys-color-outline, var(--m-color-outline, #79747e));
}

@keyframes logo-spin-cw {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@keyframes logo-spin-ccw {
  from {
    transform: rotate(360deg);
  }
  to {
    transform: rotate(0deg);
  }
}

@keyframes logo-pulse {
  0%,
  100% {
    transform: scale(1);
    opacity: 1;
  }
  50% {
    transform: scale(0.9);
    opacity: 0.8;
  }
}

@media (prefers-reduced-motion: reduce) {
  .logo-arc,
  .logo-core {
    animation: none;
  }
}
</style>
