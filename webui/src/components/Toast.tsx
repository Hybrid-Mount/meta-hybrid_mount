/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { For } from 'solid-js';
import { store } from '../lib/store';
import './Toast.css';

export default function Toast() {
  return (
    <div class="toast-container">
      <For each={store.toasts}>
        {(toast) => (
          <div
            class={`toast toast-${toast.type}`}
            role="alert"
          >
            <span>{toast.text}</span>
          </div>
        )}
      </For>
    </div>
  );
}