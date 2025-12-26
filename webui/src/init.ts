/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

declare global {
  interface Window {
    litDisableBundleWarning: boolean;
  }
}

window.litDisableBundleWarning = true;

export {};