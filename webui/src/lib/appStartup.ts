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

import type { AppAPI, InitPayload } from "./api/contracts";

type StartupApi = Pick<AppAPI, "checkInstallState" | "init">;

export type StartupGateResult =
  | { state: "cancelled" }
  | { state: "clean-reinstall-required" }
  | { state: "ready"; payload: InitPayload };

export async function runStartupGate(
  api: StartupApi,
  beginInitialize: () => boolean = () => true,
): Promise<StartupGateResult> {
  const installState = await api.checkInstallState();
  if (installState === "clean-reinstall-required") {
    return { state: installState };
  }
  if (!beginInitialize()) return { state: "cancelled" };
  return { state: "ready", payload: await api.init() };
}
