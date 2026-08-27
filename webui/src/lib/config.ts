// SPDX-License-Identifier: Apache-2.0

import type { AppConfig } from "./types";

/**
 * Clone editable configuration data without relying on structuredClone,
 * which is unavailable in some Android WebView versions.
 */
export function cloneAppConfig(source: AppConfig): AppConfig {
  return {
    ...source,
    rules: Object.fromEntries(
      Object.entries(source.rules).map(([moduleId, rule]) => [
        moduleId,
        {
          default_mode: rule.default_mode,
          paths: { ...rule.paths },
        },
      ]),
    ),
  };
}
