/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { createMemo, createSignal, Show, For } from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import { sysStore } from "../lib/stores/sysStore";
import { configStore } from "../lib/stores/configStore";
import { ICONS } from "../lib/constants";
import { ENABLE_KASUMI } from "../lib/constants_gen";
import { features } from "../lib/features";
import Skeleton from "../components/Skeleton";
import BottomActions from "../components/BottomActions";
import { API } from "../lib/api";
import { getErrorMessage } from "../lib/api/core/error";
import type { OverlayMode } from "../lib/types";
import "./StatusTab.css";

import "@material/web/iconbutton/filled-tonal-icon-button.js";
import "@material/web/icon/icon.js";
import "@material/web/dialog/dialog.js";
import "@material/web/button/text-button.js";
import "@material/web/ripple/ripple.js";

export default function StatusTab() {
  const displayPartitions = createMemo(() => [
    ...new Set(sysStore.activePartitions),
  ]);

  const [showRebootConfirm, setShowRebootConfirm] = createSignal(false);
  const isMountSourceReady = createMemo(() => !configStore.loading);
  const modeStats = createMemo(() => ({
    overlay: sysStore.storage.modeStats.overlay,
    magic: sysStore.storage.modeStats.magic,
    kasumi: sysStore.storage.modeStats.kasumi,
  }));

  const mountedCount = createMemo(() => sysStore.storage.mountedCount);

  const modeDistribution = createMemo(() => {
    const stats = modeStats();
    const showKasumi = ENABLE_KASUMI && features.kasumiEnabled;
    const overlay = stats.overlay;
    const magic = stats.magic;
    const kasumi = showKasumi ? stats.kasumi : 0;
    const total = overlay + magic + kasumi;

    if (total === 0) return { overlay: 0, magic: 0, kasumi: 0 };
    return {
      overlay: (overlay / total) * 100,
      magic: (magic / total) * 100,
      kasumi: (kasumi / total) * 100,
    };
  });

  function getModeDisplayName(mode: OverlayMode) {
    const key = `mode_${mode}` as keyof typeof uiStore.L.config;
    return uiStore.L.config[key];
  }

  return (
    <>
      <div class="dialog-container">
        <md-dialog
          open={showRebootConfirm()}
          onclose={() => setShowRebootConfirm(false)}
          class="transparent-scrim"
        >
          <div slot="headline">{uiStore.L.common.rebootTitle}</div>
          <div slot="content">{uiStore.L.common.rebootConfirm}</div>
          <div slot="actions">
            <md-text-button onClick={() => setShowRebootConfirm(false)}>
              {uiStore.L.common.cancel}
            </md-text-button>
            <md-text-button
              onClick={async () => {
                setShowRebootConfirm(false);
                try {
                  await API.reboot();
                } catch (error) {
                  uiStore.showToast(
                    getErrorMessage(error, uiStore.L.status.loadError),
                    "error",
                  );
                }
              }}
            >
              {uiStore.L.common.reboot}
            </md-text-button>
          </div>
        </md-dialog>
      </div>

      <div class="dashboard-grid">
        <div class="hero-card">
          <Show
            when={!sysStore.loading}
            fallback={
              <div class="skeleton-col">
                <Skeleton variant="hero-label" />
                <Skeleton variant="hero-title" />
                <Skeleton variant="hero-caption" />
              </div>
            }
          >
            <div class="hero-content">
              <span class="hero-label">{uiStore.L.status.storageTitle}</span>
              <span class="hero-value">
                {getModeDisplayName(sysStore.storage.type)}
              </span>
            </div>

            <div class="mount-base-chip">
              <md-icon class="mount-base-icon">
                <svg viewBox="0 0 24 24">
                  <path d={ICONS.mount_path} />
                </svg>
              </md-icon>
              <span class="mount-base-text">
                {sysStore.systemInfo.mountBase}
              </span>
            </div>
          </Show>
        </div>

        <div class="metrics-row">
          <div class="metric-card">
            <div class="metric-icon-bg">
              <svg viewBox="0 0 24 24">
                <path d={ICONS.modules} />
              </svg>
            </div>
            <span class="metric-value">{mountedCount()}</span>
            <span class="metric-label">{uiStore.L.status.moduleActive}</span>
          </div>

          <div class="metric-card">
            <Show
              when={isMountSourceReady()}
              fallback={<Skeleton variant="metric" />}
            >
              <div class="metric-icon-bg">
                <svg viewBox="0 0 24 24">
                  <path d={ICONS.ksu} />
                </svg>
              </div>
              <span class="metric-value">{configStore.config.mountsource}</span>
              <span class="metric-label">{uiStore.L.config.mountSource}</span>
            </Show>
          </div>
        </div>

        <div class="mode-stats-card">
          <div class="card-title">{uiStore.L.status.modeStats}</div>
          <div class="stats-bar-container">
            <div
              class="bar-segment bar-overlay"
              style={{ width: `${modeDistribution().overlay}%` }}
            ></div>
            <div
              class="bar-segment bar-magic"
              style={{ width: `${modeDistribution().magic}%` }}
            ></div>
            <Show when={ENABLE_KASUMI && features.kasumiEnabled}>
              <div
                class="bar-segment bar-kasumi"
                style={{ width: `${modeDistribution().kasumi}%` }}
              ></div>
            </Show>
          </div>
          <div class="stats-legend">
            <div class="legend-item">
              <div class="legend-dot dot-overlay"></div>
              <span>
                {uiStore.L.modules.modes.short.overlay +
                  ": " +
                  modeStats().overlay}
              </span>
            </div>
            <div class="legend-item">
              <div class="legend-dot dot-magic"></div>
              <span>
                {uiStore.L.modules.modes.short.magic + ": " + modeStats().magic}
              </span>
            </div>
            <Show when={ENABLE_KASUMI && features.kasumiEnabled}>
              <div class="legend-item">
                <div class="legend-dot dot-kasumi"></div>
                <span>
                  {uiStore.L.modules.modes.short.kasumi +
                    ": " +
                    modeStats().kasumi}
                </span>
              </div>
            </Show>
          </div>
        </div>

        <div class="info-card">
          <div class="card-title">{uiStore.L.status.sysInfoTitle}</div>

          <div class="info-row">
            <span class="info-key">{uiStore.L.status.kernel}</span>
            <Show
              when={!sysStore.loading}
              fallback={<Skeleton variant="info-wide" />}
            >
              <span class="info-val">{sysStore.systemInfo.kernel}</span>
            </Show>
          </div>

          <div class="info-row">
            <span class="info-key">{uiStore.L.status.selinux}</span>
            <Show
              when={!sysStore.loading}
              fallback={<Skeleton variant="info-narrow" />}
            >
              <span class="info-val">{sysStore.systemInfo.selinux}</span>
            </Show>
          </div>

          <div class="card-title card-title-spaced">
            {uiStore.L.status.activePartitions}
          </div>

          <div class="partition-list">
            <Show
              when={!sysStore.loading}
              fallback={<Skeleton variant="chip-row" />}
            >
              <For each={displayPartitions()}>
                {(part) => (
                  <div
                    class={`partition-chip ${sysStore.activePartitions.includes(part) ? "active" : ""}`}
                  >
                    {part}
                  </div>
                )}
              </For>
            </Show>
          </div>
        </div>
      </div>

      <BottomActions>
        <div class="spacer"></div>
        <div class="action-row">
          <md-filled-tonal-icon-button
            class="reboot-btn"
            onClick={() => setShowRebootConfirm(true)}
            title={uiStore.L.common.reboot}
          >
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.power} />
              </svg>
            </md-icon>
          </md-filled-tonal-icon-button>

          <md-filled-tonal-icon-button
            onClick={() => sysStore.loadStatus()}
            disabled={sysStore.loading}
            title={uiStore.L.logs.refresh}
          >
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.refresh} />
              </svg>
            </md-icon>
          </md-filled-tonal-icon-button>
        </div>
      </BottomActions>
    </>
  );
}
