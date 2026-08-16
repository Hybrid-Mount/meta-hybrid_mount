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

import { PATHS } from "../../constants";
import { UPGRADE_EPOCH } from "../../constants_gen";
import type { InstallState } from "../contracts";
import { AppError } from "../core/error";
import { readModulePropIfPresent } from "../core/bridge";

export const UPGRADE_STATE_PROPERTY = "upgradeState";
export const CLEAN_REINSTALL_STATE = "clean-reinstall-required";

function propertyValues(moduleProp: string, property: string): string[] {
  return moduleProp
    .split(/\r?\n/)
    .filter((line) => line.startsWith(`${property}=`))
    .map((line) => line.slice(property.length + 1));
}

export function parseInstallState(
  moduleProps: readonly (string | null)[],
  requiredEpoch = UPGRADE_EPOCH,
): InstallState {
  const availableProps = moduleProps.filter(
    (content): content is string => content !== null,
  );
  if (availableProps.length === 0) {
    throw new AppError("Hybrid Mount module properties are unavailable");
  }

  let cleanReinstallRequired = false;
  for (const moduleProp of availableProps) {
    const states = propertyValues(moduleProp, UPGRADE_STATE_PROPERTY);
    if (states.some((state) => state !== CLEAN_REINSTALL_STATE)) {
      throw new AppError("Hybrid Mount has an unsupported upgrade state");
    }
    if (states.length > 0) cleanReinstallRequired = true;

    const epochs = propertyValues(moduleProp, "upgradeEpoch");
    if (epochs.length !== 1 || epochs[0] !== requiredEpoch) {
      cleanReinstallRequired = true;
    }
  }

  return cleanReinstallRequired ? "clean-reinstall-required" : "ready";
}

export function previewInstallState(search: string): InstallState {
  const value = new URLSearchParams(search).get("reinstall-required");
  return value === "1" || value === "true"
    ? "clean-reinstall-required"
    : "ready";
}

export async function checkInstallState(): Promise<InstallState> {
  const moduleProps: (string | null)[] = [];
  for (const modulePath of [PATHS.MODULE_DIR, PATHS.MODULE_UPDATE_DIR]) {
    const content = await readModulePropIfPresent(modulePath);
    moduleProps.push(content);
  }
  return parseInstallState(moduleProps);
}
