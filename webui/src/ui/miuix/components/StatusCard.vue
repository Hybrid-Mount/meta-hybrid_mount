<!--

    Copyright (C) 2026 meta-magic_mount-rs developers
    SPDX-License-Identifier: GPL-v3

-->
<script setup lang="ts">
import { MiuixCard, MiuixBasicComponent, MiuixText } from "miuix-vue";

interface Props {
  status: "normal" | "checking" | "abnormal";
  label: string;
  description?: string;
  summary?: string;
}

const props = withDefaults(defineProps<Props>(), {
  status: "checking",
  label: "",
  description: "",
  summary: "",
});

const statusConfig = {
  normal: {
    bgColor: "var(--m-color-working-status-card)",
    iconColor: "var(--m-color-working-status-icon)",
    iconData:
      "m421-389-98-98q-9-9-22-9t-23 10q-9 9-9 22t9 22l122 123q9 9 21 9t21-9l239-239q10-10 10-23t-10-23q-10-9-23.5-8.5T635-603L421-389Zm59 309q-82 0-155-31.5t-127.5-86Q143-252 111.5-325T80-480q0-83 31.5-156t86-127Q252-817 325-848.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 82-31.5 155T763-197.5q-54 54.5-127 86T480-80Zm0-60q142 0 241-99.5T820-480q0-142-99-241t-241-99q-141 0-240.5 99T140-480q0 141 99.5 240.5T480-140Zm0-340Z",
  },
  checking: {
    bgColor: "var(--m-color-secondary-container)",
    iconColor: "var(--m-color-on-secondary-container)",
    iconData:
      "M310-453h340q12.75 0 21.38-8.68 8.62-8.67 8.62-21.5 0-12.82-8.62-21.32-8.63-8.5-21.38-8.5H310q-12.75 0-21.37 8.68-8.63 8.67-8.63 21.5 0 12.82 8.63 21.32 8.62 8.5 21.37 8.5ZM480.27-80q-82.74 0-155.5-31.5Q252-143 197.5-197.5t-86-127.34Q80-397.68 80-480.5t31.5-155.66Q143-709 197.5-763t127.34-85.5Q397.68-880 480.5-880t155.66 31.5Q709-817 763-763t85.5 127Q880-563 880-480.27q0 82.74-31.5 155.5Q817-252 763-197.68q-54 54.31-127 86Q563-80 480.27-80Zm.23-60Q622-140 721-239.5t99-241Q820-622 721.19-721T480-820q-141 0-240.5 98.81T140-480q0 141 99.5 240.5t241 99.5Zm-.5-340Z",
  },
  abnormal: {
    bgColor: "var(--m-color-error-container)",
    iconColor: "var(--m-color-error)",
    iconData:
      "m480-438 129 129q9 9 21 9t21-9q9-9 9-21t-9-21L522-480l129-129q9-9 9-21t-9-21q-9-9-21-9t-21 9L480-522 351-651q-9-9-21-9t-21 9q-9 9-9 21t9 21l129 129-129 129q-9 9-9 21t9 21q9 9 21 9t21-9l129-129Zm0 358q-82 0-155-31.5t-127.5-86Q143-252 111.5-325T80-480q0-83 31.5-156t86-127Q252-817 325-848.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 82-31.5 155T763-197.5q-54 54.5-127 86T480-80Zm0-60q142 0 241-99.5T820-480q0-142-99-241t-241-99q-141 0-240.5 99T140-480q0 141 99.5 240.5T480-140Zm0-340Z",
  },
};
</script>

<template>
  <MiuixCard
    class="status-card"
    press-feedback="tilt"
    :style="{
      '--m-card-color': statusConfig[props.status].bgColor,
    }"
  >
    <MiuixBasicComponent :title="props.label" :summary="description">
      <template #bottom>
        <MiuixText v-if="summary">{{ summary }}</MiuixText>
      </template>
    </MiuixBasicComponent>
    <div class="status-card__icon">
      <svg
        xmlns="http://www.w3.org/2000/svg"
        height="110px"
        width="110px"
        viewBox="0 -960 960 960"
        :fill="statusConfig[props.status].iconColor"
      >
        <path :d="statusConfig[props.status].iconData" />
      </svg>
    </div>
  </MiuixCard>
</template>

<style>
/* Additional color tokens */
:root,
.m-theme-light {
  --m-color-working-status-card: #dcfce7;
  --m-color-working-status-icon: #22c55e;
}

.m-theme-dark {
  --m-color-working-status-card: #1b3826;
  --m-color-working-status-icon: #38d167;
}

:root.miuix-monet,
.miuix-monet .m-theme-light {
  --m-color-working-status-card: var(--m-color-secondary-container);
  --m-color-working-status-icon: var(--m-color-primary);
}

.miuix-monet .m-theme-dark {
  --m-color-working-status-card: var(--m-color-secondary-container);
  --m-color-working-status-icon: var(--m-color-primary);
}

.status-card {
  padding: 0;
  overflow: visible;
  position: relative;
}
.status-card__icon {
  position: absolute;
  bottom: -32px;
  right: -22px;
}
</style>
