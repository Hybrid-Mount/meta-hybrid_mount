// SPDX-License-Identifier: Apache-2.0

import type { Module, MountMode } from "./types";

export type ModuleFilter = "active" | "all" | MountMode;

export function matchesModuleFilter(
  module: Pick<Module, "mode">,
  filter: ModuleFilter,
): boolean {
  if (filter === "active") return module.mode !== "ignore";
  if (filter === "all") return true;
  return module.mode === filter;
}
