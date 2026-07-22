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
import { API } from "../../lib/api";
import SectionShell from "./SectionShell";
import type { RuntimeSectionProps } from "./types";

export default function RuntimeSection(props: RuntimeSectionProps) {
  return (
    <SectionShell
      id="runtime"
      title={uiStore.L.kasumi.runtimeTitle}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
    >
      <div class="kasumi-config-grid">
        <button
          type="button"
          class={`kasumi-config-tile ${props.config.enable_stealth ? "active" : ""}`}
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () => API.setKasumiStealth(!props.config.enable_stealth),
              uiStore.L.kasumi.stealthUpdated,
            )
          }
        >
          <md-ripple />
          <div class="kasumi-config-icon">
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.ghost} />
              </svg>
            </md-icon>
          </div>
          <span class="kasumi-config-label">
            {uiStore.L.kasumi.stealthTitle}
          </span>
        </button>
        <button
          type="button"
          class={`kasumi-config-tile ${props.config.enable_overlay_xattr_hide ? "active" : ""}`}
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () =>
                API.setKasumiOverlayXattrHide(
                  !props.config.enable_overlay_xattr_hide,
                ),
              uiStore.L.kasumi.hidexattrUpdated,
            )
          }
        >
          <md-ripple />
          <div class="kasumi-config-icon">
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.visibility_off} />
              </svg>
            </md-icon>
          </div>
          <span class="kasumi-config-label">
            {uiStore.L.kasumi.hidexattrTitle}
          </span>
        </button>
        <button
          type="button"
          class={`kasumi-config-tile ${props.config.enable_kernel_debug ? "active" : ""}`}
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () => API.setKasumiDebug(!props.config.enable_kernel_debug),
              uiStore.L.kasumi.kernelDebugUpdated,
            )
          }
        >
          <md-ripple />
          <div class="kasumi-config-icon">
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.bug} />
              </svg>
            </md-icon>
          </div>
          <span class="kasumi-config-label">
            {uiStore.L.kasumi.kernelDebugTitle}
          </span>
        </button>
        <button
          type="button"
          class={`kasumi-config-tile ${props.config.enable_selinux_fix ? "active" : ""}`}
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () => API.setKasumiSelinuxFix(!props.config.enable_selinux_fix),
              uiStore.L.kasumi.selinuxFixUpdated,
            )
          }
        >
          <md-ripple />
          <div class="kasumi-config-icon">
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.shield} />
              </svg>
            </md-icon>
          </div>
          <span class="kasumi-config-label">
            {uiStore.L.kasumi.selinuxFixTitle}
          </span>
        </button>
      </div>
      <Show
        when={
          !props.status.available &&
          props.status.status !== "disabled" &&
          !props.lkm.loaded
        }
      >
        <div class="runtime-note warning">
          {uiStore.L.kasumi.lkmUnavailableHint}
        </div>
      </Show>
    </SectionShell>
  );
}
