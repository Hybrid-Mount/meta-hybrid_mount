import { createMemo, createSignal, onMount, Show, For } from "solid-js";
import { store } from "../lib/store";
import { ICONS } from "../lib/constants";
import { BUILTIN_PARTITIONS } from "../lib/constants_gen";
import Skeleton from "../components/Skeleton";
import BottomActions from "../components/BottomActions";
import { API } from "../lib/api";
import "./StatusTab.css";

import "@material/web/chips/chip-set.js";
import "@material/web/chips/filter-chip.js";
import "@material/web/iconbutton/filled-tonal-icon-button.js";
import "@material/web/icon/icon.js";
import "@material/web/dialog/dialog.js";
import "@material/web/button/text-button.js";

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

  function getStorageBadgeClass(type: string | null | undefined) {
    if (type === "tmpfs") return "type-tmpfs";
    if (type === "ext4") return "type-ext4";
    if (type === "erofs") return "type-erofs";
    return "";
  }

  function getModeDescription(mode: string | null | undefined) {
    if (!mode) return "";
    // @ts-ignore
    const key = `mode_${mode}Desc`;
    // @ts-ignore
    return store.L.config?.[key] || "";
  }

  function getModeDisplayName(mode: string | null | undefined) {
    if (!mode) return "Unknown";
    // @ts-ignore
    const key = `mode_${mode}`;
    // @ts-ignore
    return store.L.config?.[key] || mode.toUpperCase();
  }

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
        <div class="storage-card">
          <Show
            when={!store.loading.status}
            fallback={
              <>
                <div class="storage-header-row">
                  <div class="skeleton-col">
                    <Skeleton width="100px" height="24px" />
                    <Skeleton width="60px" height="20px" borderRadius="12px" />
                  </div>
                </div>
                <div class="storage-details">
                  <Skeleton width="100%" height="40px" />
                </div>
              </>
            }
          >
            <div
              class="storage-header-row"
              style={{ "align-items": "center", "margin-bottom": "16px" }}
            >
              <div class="storage-info-col">
                <div class="storage-label-group">
                  <div class="storage-icon-circle">
                    <svg viewBox="0 0 24 24">
                      <path d={ICONS.storage} />
                    </svg>
                  </div>
                  <span class="storage-title">
                    {store.L?.status?.storageTitle ?? "Backend"}
                  </span>
                </div>
              </div>

              <Show when={store.storage?.type}>
                <span
                  class={`storage-type-badge ${getStorageBadgeClass(store.storage.type)}`}
                >
                  {store.storage.type?.toUpperCase()}
                </span>
              </Show>
            </div>

            <div style={{ margin: "8px 0 16px 0" }}>
              <span class="storage-value" style={{ "font-size": "48px" }}>
                {getModeDisplayName(store.storage?.type)}
              </span>
            </div>

            <div
              style={{
                opacity: "0.8",
                "font-size": "14px",
                "margin-bottom": "24px",
                "line-height": "1.4",
              }}
            >
              {getModeDescription(store.storage?.type)}
            </div>

            <div
              class="storage-details"
              style={{
                background: "rgba(0,0,0,0.08)",
                padding: "12px",
                "border-radius": "12px",
              }}
            >
              <span style={{ opacity: "0.7", "font-size": "12px" }}>
                {store.L?.status?.mountBase ?? "Mount Base"}
              </span>
              <span
                class="detail-path"
                style={{
                  "max-width": "100%",
                  background: "transparent",
                  padding: "0",
                }}
              >
                {store.systemInfo?.mountBase ?? "Unknown"}
              </span>
            </div>
          </Show>
        </div>

        <div class="stats-row">
          <div class="stat-card">
            <Show
              when={!store.loading.status}
              fallback={
                <>
                  <Skeleton width="40px" height="32px" />
                  <Skeleton width="60px" height="12px" class="mt-8" />
                </>
              }
            >
              <div class="stat-value">{mountedCount()}</div>
              <div class="stat-label">
                {store.L?.status?.moduleActive ?? "Active Modules"}
              </div>
            </Show>
          </div>
          <div class="stat-card">
            <Show
              when={!store.loading.status}
              fallback={
                <>
                  <Skeleton width="40px" height="32px" />
                  <Skeleton width="60px" height="12px" class="mt-8" />
                </>
              }
            >
              <div class="stat-value">{store.config?.mountsource ?? "-"}</div>
              <div class="stat-label">
                {store.L?.config?.mountSource ?? "Mount Source"}
              </div>
            </Show>
          </div>
        </div>

        <div class="mode-card">
          <div class="mode-title">
            {store.L?.status?.activePartitions ?? "Partitions"}
          </div>

          <Show
            when={!store.loading.status}
            fallback={
              <div class="partition-grid">
                <For each={Array(4)}>
                  {() => (
                    <Skeleton width="60px" height="32px" borderRadius="8px" />
                  )}
                </For>
              </div>
            }
          >
            <md-chip-set class="partition-chips">
              <For each={displayPartitions()}>
                {(part) => (
                  <md-filter-chip
                    label={part}
                    selected={(store.activePartitions || []).includes(part)}
                    elevated
                  ></md-filter-chip>
                )}
              </For>
            </md-chip-set>
          </Show>
        </div>

        <div class="mode-card">
          <div class="mode-title">
            {store.L?.status?.sysInfoTitle ?? "System Info"}
          </div>
          <div class="info-grid">
            <div class="info-item">
              <span class="info-label">
                {store.L?.status?.kernel ?? "Kernel"}
              </span>
              <Show
                when={!store.loading.status}
                fallback={<Skeleton width="80%" height="16px" />}
              >
                <span class="info-val">{store.systemInfo?.kernel || "-"}</span>
              </Show>
            </div>
            <div class="info-item">
              <span class="info-label">
                {store.L?.status?.selinux ?? "SELinux"}
              </span>
              <Show
                when={!store.loading.status}
                fallback={<Skeleton width="40%" height="16px" />}
              >
                <span class="info-val">{store.systemInfo?.selinux || "-"}</span>
              </Show>
            </div>
          </div>
        </div>

        <div class="mode-card">
          <div class="mode-title mb-8">
            {store.L?.status?.modeStats ?? "Mode Stats"}
          </div>
          <Show
            when={!store.loading.status}
            fallback={
              <div class="skeleton-group">
                <Skeleton width="100%" height="20px" />
                <Skeleton width="100%" height="20px" />
                <Skeleton width="100%" height="20px" />
              </div>
            }
          >
            <div class="mode-row">
              <div class="mode-name">
                <div class="dot secondary"></div>
                {store.L?.status?.modeAuto ?? "Auto"}
              </div>
              <span class="mode-count">{store.modeStats?.auto ?? 0}</span>
            </div>
            <div class="mode-divider"></div>
            <div class="mode-row">
              <div class="mode-name">
                <div class="dot tertiary"></div>
                {store.L?.status?.modeMagic ?? "Magic"}
              </div>
              <span class="mode-count">{store.modeStats?.magic ?? 0}</span>
            </div>
          </Show>
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
