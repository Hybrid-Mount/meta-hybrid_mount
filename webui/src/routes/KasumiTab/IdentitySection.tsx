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
import { uiStore } from "../../lib/stores/uiStore";
import { API } from "../../lib/api";
import SectionShell from "./SectionShell";
import type { IdentitySectionProps } from "./types";

export default function IdentitySection(props: IdentitySectionProps) {
  return (
    <SectionShell
      id="identity"
      title={uiStore.L.kasumi.identityTitle}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
      badge={
        props.unameMode === "global"
          ? uiStore.L.kasumi.unameModeGlobal
          : uiStore.L.kasumi.unameModeScoped
      }
      badgeActive={props.unameMode === "global"}
    >
      <div class="field-stack">
        <div class="uname-panel">
          <div class="uname-panel-head">
            <div>
              <div class="uname-panel-title">
                {uiStore.L.kasumi.unameSpoofTitle}
              </div>
              <div class="uname-panel-subtitle">
                {props.release || props.version
                  ? `${props.release || "-"} · ${props.version || "-"}`
                  : uiStore.L.kasumi.unameEmptyHint}
              </div>
            </div>
          </div>

          <div class="uname-field-grid">
            <md-outlined-text-field
              class="full-field kasumi-input-field"
              label={uiStore.L.kasumi.unameRelease}
              value={props.release}
              supporting-text={uiStore.L.kasumi.unameReleaseDesc}
              onInput={(e: Event) =>
                props.setRelease((e.currentTarget as HTMLInputElement).value)
              }
              disabled={props.pending}
            />
            <md-outlined-text-field
              class="full-field kasumi-input-field"
              label={uiStore.L.kasumi.unameVersion}
              value={props.version}
              supporting-text={uiStore.L.kasumi.unameVersionDesc}
              onInput={(e: Event) =>
                props.setVersion((e.currentTarget as HTMLInputElement).value)
              }
              disabled={props.pending}
            />
          </div>

          <div class="uname-mode-row">
            <div class="uname-segmented" role="radiogroup">
              <button
                type="button"
                class={props.unameMode === "scoped" ? "selected" : ""}
                disabled={props.pending}
                onClick={() => props.setUnameMode("scoped")}
              >
                {uiStore.L.kasumi.unameModeScoped}
              </button>
              <button
                type="button"
                class={props.unameMode === "global" ? "selected" : ""}
                disabled={props.pending}
                onClick={() => props.setUnameMode("global")}
              >
                {uiStore.L.kasumi.unameModeGlobal}
              </button>
            </div>
            <div class="uname-mode-desc">{props.unameModeDescription}</div>
          </div>

          <div class="button-row">
            <md-outlined-button
              disabled={props.pending}
              onClick={() => void props.fillOriginalKernelUname()}
            >
              {uiStore.L.kasumi.fillOriginalKernel}
            </md-outlined-button>
            <md-filled-button
              disabled={
                props.pending || !props.release.trim() || !props.version.trim()
              }
              onClick={() =>
                props.runAction(
                  props.saveAndApplyUname,
                  uiStore.L.kasumi.applyUname,
                )
              }
            >
              {uiStore.L.kasumi.applyUname}
            </md-filled-button>
            <md-outlined-button
              disabled={props.pending}
              onClick={() =>
                props.runAction(props.clearUname, uiStore.L.kasumi.clearUname)
              }
            >
              {uiStore.L.kasumi.clearUname}
            </md-outlined-button>
            <Show when={props.unameMode === "global"}>
              <md-outlined-button
                disabled={props.pending}
                onClick={() =>
                  props.runAction(
                    () => API.restoreKasumiUnameGlobal(),
                    uiStore.L.kasumi.restoreUnameGlobal,
                  )
                }
              >
                {uiStore.L.kasumi.restoreUnameGlobal}
              </md-outlined-button>
            </Show>
          </div>
        </div>

        <md-outlined-text-field
          class="full-field kasumi-input-field"
          label={uiStore.L.kasumi.cmdlineValue}
          value={props.cmdline}
          onInput={(e: Event) =>
            props.setCmdline((e.currentTarget as HTMLInputElement).value)
          }
          disabled={props.pending}
        />
        <div class="button-row">
          <md-filled-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => API.setKasumiCmdline(props.cmdline),
                uiStore.L.common.saved,
              )
            }
          >
            {uiStore.L.kasumi.saveCmdline}
          </md-filled-button>
          <md-outlined-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => API.clearKasumiCmdline(),
                uiStore.L.kasumi.clearCmdline,
              )
            }
          >
            {uiStore.L.kasumi.clearCmdline}
          </md-outlined-button>
        </div>
      </div>
    </SectionShell>
  );
}
