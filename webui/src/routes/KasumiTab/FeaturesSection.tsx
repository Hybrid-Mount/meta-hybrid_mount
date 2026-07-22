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

import { For, Show } from "solid-js";
import { uiStore } from "../../lib/stores/uiStore";
import Skeleton from "../../components/Skeleton";
import SectionShell from "./SectionShell";
import type { FeaturesSectionProps } from "./types";

export default function FeaturesSection(props: FeaturesSectionProps) {
  return (
    <SectionShell
      id="features"
      title={uiStore.L.kasumi.featuresTitle}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
    >
      <Show
        when={!props.loading}
        fallback={<Skeleton variant="feature-card" />}
      >
        <div class="meta-list">
          <div class="meta-row">
            <span>{uiStore.L.kasumi.featureBits}</span>
            <strong>{props.status.feature_bits ?? 0}</strong>
          </div>
          <div class="meta-row">
            <span>{uiStore.L.kasumi.hideUidCount}</span>
            <strong>{props.config.hide_uids.length}</strong>
          </div>
          <div class="meta-row">
            <span>{uiStore.L.kasumi.userHideCount}</span>
            <strong>{props.status.user_hide_rule_count}</strong>
          </div>
          <div class="meta-row">
            <span>{uiStore.L.kasumi.mapsRuleCount}</span>
            <strong>{props.config.maps_rules.length}</strong>
          </div>
          <div class="meta-row">
            <span>{uiStore.L.kasumi.kstatRuleCount}</span>
            <strong>{props.config.kstat_rules.length}</strong>
          </div>
        </div>
        <div class="chip-section">
          <For each={props.status.feature_names}>
            {(name) => <span class="feature-chip">{name}</span>}
          </For>
        </div>
        <div class="chip-section subdued">
          <For each={props.status.hooks}>
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
