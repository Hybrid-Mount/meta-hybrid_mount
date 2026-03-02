/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { For } from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import "./Toast.css";

export default function Toast() {
  return (
    <div class="toast-container">
      <For each={uiStore.toasts}>
        {(toast) => (
          <div class={`toast toast-${toast.type}`} role="alert">
            <span>{toast.text}</span>
          </div>
        )}
      </For>
    </div>
  );
}
