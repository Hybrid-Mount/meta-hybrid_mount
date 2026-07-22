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
import { API } from "../../lib/api";
import SectionShell from "./SectionShell";
import { parseUnsignedInput } from "./utils";
import type { MapsSectionProps } from "./types";

export default function MapsSection(props: MapsSectionProps) {
  return (
    <SectionShell
      id="maps"
      title={uiStore.L.kasumi.mapsTitle}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
      badge={String(props.config.maps_rules.length)}
    >
      <div class="field-stack">
        <div class="meta-list">
          <div class="meta-row">
            <span>{uiStore.L.kasumi.mapsRuleCount}</span>
            <strong>{props.config.maps_rules.length}</strong>
          </div>
        </div>
        <div class="sub-grid">
          <md-outlined-text-field
            class="full-field kasumi-input-field"
            label={uiStore.L.kasumi.mapsTargetIno}
            value={props.mapsTargetIno}
            onInput={(e: Event) =>
              props.setMapsTargetIno(
                (e.currentTarget as HTMLInputElement).value,
              )
            }
            disabled={props.pending}
          />
          <md-outlined-text-field
            class="full-field kasumi-input-field"
            label={uiStore.L.kasumi.mapsTargetDev}
            value={props.mapsTargetDev}
            onInput={(e: Event) =>
              props.setMapsTargetDev(
                (e.currentTarget as HTMLInputElement).value,
              )
            }
            disabled={props.pending}
          />
          <md-outlined-text-field
            class="full-field kasumi-input-field"
            label={uiStore.L.kasumi.mapsSpoofedIno}
            value={props.mapsSpoofedIno}
            onInput={(e: Event) =>
              props.setMapsSpoofedIno(
                (e.currentTarget as HTMLInputElement).value,
              )
            }
            disabled={props.pending}
          />
          <md-outlined-text-field
            class="full-field kasumi-input-field"
            label={uiStore.L.kasumi.mapsSpoofedDev}
            value={props.mapsSpoofedDev}
            onInput={(e: Event) =>
              props.setMapsSpoofedDev(
                (e.currentTarget as HTMLInputElement).value,
              )
            }
            disabled={props.pending}
          />
        </div>
        <md-outlined-text-field
          class="full-field kasumi-input-field"
          label={uiStore.L.kasumi.mapsSpoofedPath}
          value={props.mapsPath}
          onInput={(e: Event) =>
            props.setMapsPath((e.currentTarget as HTMLInputElement).value)
          }
          disabled={props.pending}
        />
        <div class="button-row">
          <md-filled-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(() => {
                const spoofedPath = props.mapsPath.trim();
                if (!spoofedPath) {
                  throw new Error(uiStore.L.kasumi.mapsPathRequired);
                }
                return API.addKasumiMapsRule({
                  target_ino: parseUnsignedInput(
                    props.mapsTargetIno,
                    "target inode",
                  ),
                  target_dev: parseUnsignedInput(
                    props.mapsTargetDev,
                    "target device",
                  ),
                  spoofed_ino: parseUnsignedInput(
                    props.mapsSpoofedIno,
                    "spoofed inode",
                  ),
                  spoofed_dev: parseUnsignedInput(
                    props.mapsSpoofedDev,
                    "spoofed device",
                  ),
                  spoofed_pathname: spoofedPath,
                });
              }, uiStore.L.kasumi.mapsRuleAdded)
            }
          >
            {uiStore.L.kasumi.mapsAddRule}
          </md-filled-button>
          <md-outlined-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => API.clearKasumiMapsRules(),
                uiStore.L.kasumi.mapsCleared,
              )
            }
          >
            {uiStore.L.kasumi.mapsClear}
          </md-outlined-button>
        </div>
        <div class="hide-rule-list">
          <For each={props.config.maps_rules}>
            {(rule) => (
              <div class="hide-rule-item">
                <div class="hide-rule-path">
                  <div class="mono">{rule.spoofed_pathname}</div>
                  <div class="secondary-inline mono">
                    {uiStore.L.kasumi.mapsRuleSummary
                      .replace(
                        "{target}",
                        `${rule.target_ino}:${rule.target_dev}`,
                      )
                      .replace(
                        "{spoofed}",
                        `${rule.spoofed_ino}:${rule.spoofed_dev}`,
                      )}
                  </div>
                </div>
              </div>
            )}
          </For>
          <Show when={props.config.maps_rules.length === 0}>
            <div class="empty-inline-note">{uiStore.L.kasumi.mapsEmpty}</div>
          </Show>
        </div>
      </div>
    </SectionShell>
  );
}
