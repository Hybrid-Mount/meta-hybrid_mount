import { For, Show } from "solid-js";
import { uiStore } from "../../lib/stores/uiStore";
import Skeleton from "../../components/Skeleton";
import SectionShell from "./SectionShell";
import type { FeaturesSectionProps } from "./types";

export default function FeaturesSection(props: FeaturesSectionProps) {
  return (
    <SectionShell
      id="features"
      title={uiStore.L.kasumi?.featuresTitle ?? "Capabilities"}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
    >
      <Show
        when={!props.loading}
        fallback={<Skeleton variant="feature-card" />}
      >
        <div class="meta-list">
          <div class="meta-row">
            <span>{uiStore.L.kasumi?.featureBits ?? "Feature bits"}</span>
            <strong>{props.status?.feature_bits ?? 0}</strong>
          </div>
          <div class="meta-row">
            <span>{uiStore.L.kasumi?.hideUidCount ?? "Hide UIDs"}</span>
            <strong>{props.config?.hide_uids?.length ?? 0}</strong>
          </div>
          <div class="meta-row">
            <span>{uiStore.L.kasumi?.userHideCount ?? "User hide rules"}</span>
            <strong>{props.status?.user_hide_rule_count ?? 0}</strong>
          </div>
          <div class="meta-row">
            <span>{uiStore.L.kasumi?.mapsRuleCount ?? "Maps rules"}</span>
            <strong>{props.config?.maps_rules?.length ?? 0}</strong>
          </div>
          <div class="meta-row">
            <span>{uiStore.L.kasumi?.kstatRuleCount ?? "Kstat rules"}</span>
            <strong>{props.config?.kstat_rules?.length ?? 0}</strong>
          </div>
        </div>
        <div class="chip-section">
          <For each={props.status?.feature_names || []}>
            {(name) => <span class="feature-chip">{name}</span>}
          </For>
        </div>
        <div class="chip-section subdued">
          <For each={props.status?.hooks || []}>
            {(name) => <span class="feature-chip hook">{name}</span>}
          </For>
        </div>
        <div class="chip-section">
          <For each={props.activeModules}>
            {(name) => <span class="feature-chip active-module">{name}</span>}
          </For>
        </div>
      </Show>
    </SectionShell>
  );
}
