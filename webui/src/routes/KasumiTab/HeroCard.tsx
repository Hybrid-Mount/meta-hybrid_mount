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
          <span class="hero-label">{uiStore.L.kasumi.title}</span>
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
