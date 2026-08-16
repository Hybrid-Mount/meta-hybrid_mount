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

import { ICONS } from "../lib/constants";
import { uiStore } from "../lib/stores/uiStore";
import "./CleanReinstallRequired.css";

export default function CleanReinstallRequired() {
  return (
    <main class="clean-reinstall-page">
      <article
        class="clean-reinstall-card"
        aria-labelledby="clean-reinstall-title"
      >
        <div class="clean-reinstall-heading" role="alert">
          <div class="clean-reinstall-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d={ICONS.warning} />
            </svg>
          </div>
          <div>
            <p class="clean-reinstall-eyebrow">
              {uiStore.L.compatibility.eyebrow}
            </p>
            <h1 id="clean-reinstall-title">{uiStore.L.compatibility.title}</h1>
          </div>
        </div>

        <p class="clean-reinstall-description">
          {uiStore.L.compatibility.description}
        </p>

        <section
          class="clean-reinstall-steps"
          aria-labelledby="clean-reinstall-steps-title"
        >
          <h2 id="clean-reinstall-steps-title">
            {uiStore.L.compatibility.stepsTitle}
          </h2>
          <ol role="list">
            <li>{uiStore.L.compatibility.uninstallStep}</li>
            <li>{uiStore.L.compatibility.rebootStep}</li>
            <li>{uiStore.L.compatibility.reinstallStep}</li>
          </ol>
        </section>

        <div class="clean-reinstall-warning">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d={ICONS.warning} />
          </svg>
          <p>{uiStore.L.compatibility.dataWarning}</p>
        </div>
      </article>
    </main>
  );
}
