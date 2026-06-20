<!--

    Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.

-->
<template>
  <div v-if="!dev_mode" class="app-logo">
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 120 120"
      class="dev-logo"
    >
      <circle cx="60" cy="60" r="50" class="logo-base-track" />
      <circle cx="60" cy="60" r="38" class="logo-base-track" />
      <circle cx="60" cy="60" r="26" class="logo-base-track" />

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
        <path
          d="M 60 34 A 26 26 0 1 1 47 82.5"
          class="logo-arc logo-arc-inner"
        />
      </g>

      <circle cx="60" cy="60" r="10" class="logo-core" />
    </svg>
  </div>
  <div v-else class="app-logo">
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120">
      <circle cx="60" cy="60" r="50" class="logo-base-track" />
      <circle cx="60" cy="60" r="38" class="logo-base-track" />
      <circle cx="60" cy="60" r="26" class="logo-base-track" />

      <path d="M60 10 A 50 50 0 0 1 110 60" class="logo-arc logo-arc-outer" />
      <path d="M60 98 A 38 38 0 0 1 60 22" class="logo-arc logo-arc-mid" />
      <path d="M34 60 A 26 26 0 1 1 86 60" class="logo-arc logo-arc-inner" />

      <circle cx="60" cy="60" r="10" class="logo-core" />
    </svg>
  </div>
</template>

<style>
.app-logo {
  width: 120px;
  height: 120px;
}

.app-logo svg {
  width: 100%;
  height: 100%;
  overflow: visible;
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

@keyframes logo-pulse-fast {
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

.logo-base-track {
  fill: none;
  stroke: var(--m-color-surface-variant);
  opacity: 0.3;
  stroke-width: 1;
}

.logo-core {
  fill: var(--m-color-primary);
  transform-origin: center;
  animation: logo-pulse-fast 2s ease-in-out infinite;
  filter: drop-shadow(0 0 6px var(--m-color-primary-container));
}

.logo-arc {
  fill: none;
  stroke-linecap: round;
  transform-origin: center;
}

.logo-arc-outer {
  stroke: var(--m-color-on-tertiary-container);
  stroke-width: 4;
  animation: logo-spin-cw 12s linear infinite;
}

.logo-arc-mid {
  stroke: var(--m-color-secondary);
  stroke-width: 5;
  animation: logo-spin-ccw 8s ease-in-out infinite;
}

.logo-arc-inner {
  stroke: var(--m-color-primary);
  stroke-width: 6;
  animation: logo-spin-cw 4s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

.dev-logo .logo-base-track {
  stroke-width: 8px;
  opacity: 0.1;
}

.dev-logo .logo-arc {
  stroke-width: 8px;
}

.dev-logo .logo-arc-error {
  stroke: var(--m-color-error);
  stroke-dasharray: 10 14;
}

.dev-logo .logo-arc-inner {
  stroke: var(--m-color-outline);
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
</style>

<script setup lang="ts">
import { ref } from "vue";
import { IS_RELEASE } from "../lib/constants_gen";

const dev_mode = ref(IS_RELEASE);
</script>

export default { name: 'Logo', }
