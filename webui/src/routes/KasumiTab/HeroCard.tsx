import { Show } from "solid-js";
import { ICONS } from "../../lib/constants";
import { uiStore } from "../../lib/stores/uiStore";
import Skeleton from "../../components/Skeleton";

export default function HeroCard(props: {
  loading: boolean;
  heroStatusText: string;
  heroSubtitleText: string;
  statusChipText: string;
}) {
  return (
    <section class="hero-card kasumi-status-card">
      <Show
        when={!props.loading}
        fallback={
          <div class="skeleton-col">
            <Skeleton variant="hero-label" />
            <Skeleton variant="hero-title" />
            <Skeleton variant="hero-caption" />
          </div>
        }
      >
        <div class="hero-content">
          <span class="hero-label">
            {uiStore.L.kasumi?.title ?? "Kasumi Runtime"}
          </span>
          <span class="hero-value">{props.heroStatusText}</span>
          <span class="kasumi-hero-caption">{props.heroSubtitleText}</span>
        </div>

        <div class="mount-base-chip">
          <md-icon class="mount-base-icon">
            <svg viewBox="0 0 24 24">
              <path d={ICONS.mount_path} />
            </svg>
          </md-icon>
          <span class="mount-base-text">{props.statusChipText}</span>
        </div>
      </Show>
    </section>
  );
}
