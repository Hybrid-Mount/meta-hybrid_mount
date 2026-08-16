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

import type { StorageStatus } from "../../types";
import type { RuntimeStatePayload } from "../schemas";

export function buildModeStats(
  state: RuntimeStatePayload,
): NonNullable<StorageStatus["modeStats"]> {
  const ms = state.mode_stats;
  return {
    overlay: ms?.overlayfs ?? 0,
    magic: ms?.magicmount ?? 0,
    blacklisted: ms?.blacklisted ?? 0,
  };
}

export function buildMountedCount(
  state: RuntimeStatePayload,
  modeStats: NonNullable<StorageStatus["modeStats"]>,
): number {
  const overlay = state.overlay_modules?.length ?? 0;
  const magic = state.magic_modules?.length ?? 0;
  const total = overlay + magic;
  return total > 0 ? total : modeStats.overlay + modeStats.magic;
}
