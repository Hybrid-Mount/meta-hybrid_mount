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

import type { KasumiStatus, StorageStatus } from "../../types";
import type { KasumiStatusPayload, RuntimeStatePayload } from "../schemas";

export function buildModeStats(
  state: RuntimeStatePayload,
): NonNullable<StorageStatus["modeStats"]> {
  return {
    overlay: state.mode_stats.overlayfs,
    magic: state.mode_stats.magicmount,
    kasumi: state.mode_stats.kasumi,
    blacklisted: state.mode_stats.blacklisted,
  };
}

export function buildMountedCount(state: RuntimeStatePayload): number {
  return (
    state.overlay_modules.length +
    state.magic_modules.length +
    state.kasumi_modules.length
  );
}

export function buildKasumiStatusFromPayload(
  payload: KasumiStatusPayload,
): KasumiStatus {
  return payload as KasumiStatus;
}
