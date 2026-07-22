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
import type { UserHideSectionProps } from "./types";

export default function UserHideSection(props: UserHideSectionProps) {
  return (
    <SectionShell
      id="user-hide"
      title={uiStore.L.kasumi.userHideTitle}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
    >
      <div class="field-stack">
        <md-outlined-text-field
          class="full-field kasumi-input-field"
          label={uiStore.L.kasumi.userHidePathLabel}
          value={props.userHidePath}
          onInput={(e: Event) =>
            props.setUserHidePath((e.currentTarget as HTMLInputElement).value)
          }
          disabled={props.pending}
        />
        <div class="button-row">
          <md-filled-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => {
                  const path = props.userHidePath.trim();
                  if (!path) {
                    throw new Error(uiStore.L.kasumi.userHidePathRequired);
                  }
                  return API.addUserHideRule(path);
                },
                uiStore.L.kasumi.hideRuleAdded,
                "full",
              )
            }
          >
            {uiStore.L.kasumi.addHideRule}
          </md-filled-button>
          <md-outlined-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => API.applyUserHideRules(),
                uiStore.L.kasumi.hideRulesApplied,
                "full",
              )
            }
          >
            {uiStore.L.kasumi.applyHideRules}
          </md-outlined-button>
        </div>
        <div class="hide-rule-list">
          <For each={props.userHideRules}>
            {(path) => (
              <div class="hide-rule-item">
                <span class="hide-rule-path mono">{path}</span>
                <button
                  class="hide-rule-remove"
                  type="button"
                  disabled={props.pending}
                  onClick={() =>
                    props.runAction(
                      () => API.removeUserHideRule(path),
                      uiStore.L.kasumi.hideRuleRemoved,
                      "full",
                    )
                  }
                >
                  {uiStore.L.kasumi.removeHideRule}
                </button>
              </div>
            )}
          </For>
          <Show when={props.userHideRules.length === 0}>
            <div class="empty-inline-note">
              {uiStore.L.kasumi.noUserHideRules}
            </div>
          </Show>
        </div>
      </div>
    </SectionShell>
  );
}
