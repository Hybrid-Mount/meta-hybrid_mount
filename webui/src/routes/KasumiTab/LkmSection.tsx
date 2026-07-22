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

import { uiStore } from "../../lib/stores/uiStore";
import { API } from "../../lib/api";
import SectionShell from "./SectionShell";
import type { LkmSectionProps } from "./types";

export default function LkmSection(props: LkmSectionProps) {
  const autoloadText = props.lkm.autoload
    ? uiStore.L.kasumi.autoloadOn
    : uiStore.L.kasumi.autoloadOff;

  return (
    <SectionShell
      id="lkm"
      title={uiStore.L.kasumi.lkmTitle}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
      badge={autoloadText}
      badgeActive={props.lkm.autoload}
    >
      <div class="meta-list">
        <div class="meta-row">
          <span>{uiStore.L.kasumi.currentKmi}</span>
          <strong>{props.lkm.current_kmi}</strong>
        </div>
      </div>
      <div class="field-row">
        <button
          class="kasumi-select-button"
          type="button"
          disabled={props.pending}
          onClick={props.onShowKmiDialog}
        >
          <div class="kasumi-select-button-label">
            {uiStore.L.kasumi.kmiOverride}
          </div>
          <div class="kasumi-select-button-value">
            {props.kmi || uiStore.L.kasumi.autoKmi}
          </div>
        </button>
      </div>
      <div class="button-row">
        <md-filled-button
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () => API.setKasumiLkmKmi(props.kmi),
              uiStore.L.kasumi.saveKmi,
            )
          }
        >
          {uiStore.L.kasumi.saveKmi}
        </md-filled-button>
      </div>
      <div class="button-row">
        <md-outlined-button
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () => API.setKasumiLkmAutoload(!props.lkm.autoload),
              uiStore.L.kasumi.autoloadUpdated,
            )
          }
        >
          {props.lkm.autoload
            ? uiStore.L.kasumi.disableAutoload
            : uiStore.L.kasumi.enableAutoload}
        </md-outlined-button>
        <md-filled-button
          disabled={props.pending}
          onClick={() =>
            props.lkm.loaded
              ? props.onShowUnloadWarning()
              : props.runAction(
                  () => API.loadKasumiLkm(),
                  uiStore.L.kasumi.loadLkm,
                )
          }
        >
          {props.lkm.loaded
            ? uiStore.L.kasumi.unloadLkm
            : uiStore.L.kasumi.loadLkm}
        </md-filled-button>
      </div>
    </SectionShell>
  );
}
