/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createMemo, createSignal, onMount, Show, For } from "solid-js";
import { store } from "../lib/store";
import { ICONS } from "../lib/constants";
import { BUILTIN_PARTITIONS } from "../lib/constants_gen";
import Skeleton from "../components/Skeleton";
import BottomActions from "../components/BottomActions";
import { API } from "../lib/api";
import "./StatusTab.css";

import "@material/web/iconbutton/filled-tonal-icon-button.js";
import "@material/web/icon/icon.js";
import "@material/web/dialog/dialog.js";
import "@material/web/button/text-button.js";
import "@material/web/ripple/ripple.js";

export default function StatusTab() {
  onMount(() => {
    store.loadStatus();
  });

  const displayPartitions = createMemo(() => [
    ...new Set([...BUILTIN_PARTITIONS, ...(store.config?.partitions || [])]),
  ]);

  const mountedCount = createMemo(
    () => store.modules?.filter((m) => m.is_mounted).length ?? 0,
  );

  const [showRebootConfirm, setShowRebootConfirm] = createSignal(false);

  function getModeDisplayName(mode: string | null | undefined) {
    if (!mode) return "Unknown";
    // @ts-ignore
    const key = `mode_${mode}`;
    // @ts-ignore
    return store.L.config?.[key] || mode.toUpperCase();
  }

  const modeDistribution = createMemo(() => {
    const stats = store.modeStats;
    const total = (stats?.auto || 0) + (stats?.magic || 0);
    if (total === 0) return { auto: 0, magic: 0 };
    return {
      auto: (stats.auto / total) * 100,
      magic: (stats.magic / total) * 100,
    };
  });

  return (
    <>
      <md-dialog
        open={showRebootConfirm()}
        onclose={() => setShowRebootConfirm(false)}
        class="transparent-scrim"
      >
        <div slot="headline">
          {store.L?.common?.rebootTitle ?? "Reboot System?"}
        </div>
        <div slot="content">
          {store.L?.common?.rebootConfirm ??
            "Are you sure you want to reboot the device?"}
        </div>
        <div slot="actions">
          <md-text-button onClick={() => setShowRebootConfirm(false)}>
            {store.L?.common?.cancel ?? "Cancel"}
          </md-text-button>
          <md-text-button
            onClick={() => {
              setShowRebootConfirm(false);
              API.reboot();
            }}
          >
            {store.L?.common?.reboot ?? "Reboot"}
          </md-text-button>
        </div>
      </md-dialog>

      <div class="dashboard-grid">
        <div class="hero-card">
          <Show
            when={!store.loading.status}
            fallback={
              <div class="skeleton-col">
                <Skeleton width="40%" height="24px" />
                <Skeleton width="80%" height="48px" />
                <Skeleton width="60%" height="20px" />
              </div>
            }
          >
            <div class="hero-header">
              <div class="storage-badge">
                <md-icon
                  style={{
                    "font-size": "16px",
                    width: "16px",
                    height: "16px",
                  }}
                >
                  <svg viewBox="0 0 24 24">
                    <path d={ICONS.storage} />
                  </svg>
                </md-icon>
                <span>{store.storage?.type || "Unknown"}</span>
              </div>
            </div>

            <div class="hero-content">
              <span class="hero-label">
                {store.L?.status?.storageTitle ?? "Backend Strategy"}
              </span>
              <span class="hero-value">
                {getModeDisplayName(store.storage?.type)}
              </span>
            </div>

            <div class="mount-base-chip">
              <md-icon
                style={{
                  "font-size": "14px",
                  width: "14px",
                  height: "14px",
                  opacity: 0.7,
                }}
              >
                <svg viewBox="0 0 24 24">
                  <path d={ICONS.mount_path} />
                </svg>
              </md-icon>
              <span class="mount-base-text">
                {store.systemInfo?.mountBase || "Unknown"}
              </span>
            </div>
          </Show>
        </div>

        <div class="metrics-row">
          <div class="metric-card">
            <Show
              when={!store.loading.status}
              fallback={<Skeleton width="50%" height="32px" />}
            >
              <div class="metric-icon-bg">
                <svg viewBox="0 0 24 24">
                  <path d={ICONS.modules} />
                </svg>
              </div>
              <span class="metric-value">{mountedCount()}</span>
              <span class="metric-label">
                {store.L?.status?.moduleActive ?? "Active Modules"}
              </span>
            </Show>
          </div>

          <div class="metric-card">
            <Show
              when={!store.loading.status}
              fallback={<Skeleton width="50%" height="32px" />}
            >
              <div class="metric-icon-bg">
                <svg viewBox="0 0 24 24">
                  <path d={ICONS.ksu} />
                </svg>
              </div>
              <span class="metric-value">
                {store.config?.mountsource || "-"}
              </span>
              <span class="metric-label">
                {store.L?.config?.mountSource ?? "Mount Source"}
              </span>
            </Show>
          </div>
        </div>

        <div class="mode-stats-card">
          <div class="card-title">
            {store.L?.status?.modeStats ?? "Mode Distribution"}
          </div>
          <Show
            when={!store.loading.status}
            fallback={
              <Skeleton width="100%" height="24px" borderRadius="12px" />
            }
          >
            <div class="stats-bar-container">
              <div
                class="bar-segment bar-auto"
                style={{ width: `${modeDistribution().auto}%` }}
              ></div>
              <div
                class="bar-segment bar-magic"
                style={{ width: `${modeDistribution().magic}%` }}
              ></div>
            </div>
            <div class="stats-legend">
              <div class="legend-item">
                <div class="legend-dot dot-auto"></div>
                <span>Overlay: {store.modeStats?.auto || 0}</span>
              </div>
              <div class="legend-item">
                <div class="legend-dot dot-magic"></div>
                <span>Magic: {store.modeStats?.magic || 0}</span>
              </div>
            </div>
          </Show>
        </div>

        <div class="info-card">
          <div class="card-title">
            {store.L?.status?.sysInfoTitle ?? "System Info"}
          </div>

          <div class="info-row">
            <span class="info-key">{store.L?.status?.kernel ?? "Kernel"}</span>
            <Show
              when={!store.loading.status}
              fallback={<Skeleton width="100px" height="16px" />}
            >
              <span class="info-val">{store.systemInfo?.kernel || "-"}</span>
            </Show>
          </div>

          <div class="info-row">
            <span class="info-key">
              {store.L?.status?.selinux ?? "SELinux"}
            </span>
            <Show
              when={!store.loading.status}
              fallback={<Skeleton width="60px" height="16px" />}
            >
              <span class="info-val">{store.systemInfo?.selinux || "-"}</span>
            </Show>
          </div>

          <div class="card-title" style={{ "margin-top": "8px" }}>
            {store.L?.status?.activePartitions ?? "Partitions"}
          </div>

          <div class="partition-list">
            <Show
              when={!store.loading.status}
              fallback={<Skeleton width="100%" height="32px" />}
            >
              <For each={displayPartitions()}>
                {(part) => (
                  <div
                    class={`partition-chip ${(store.activePartitions || []).includes(part) ? "active" : ""}`}
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
            title="Reboot"
            role="button"
            tabIndex={0}
          >
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.power} />
              </svg>
            </md-icon>
          </md-filled-tonal-icon-button>

          <md-filled-tonal-icon-button
            onClick={() => store.loadStatus()}
            disabled={store.loading.status}
            title={store.L?.logs?.refresh}
            role="button"
            tabIndex={0}
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
