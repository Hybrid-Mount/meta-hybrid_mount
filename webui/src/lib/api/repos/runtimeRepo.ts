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

export interface RuntimeStatePayload {
  pid?: number;
  storage_mode?: string;
  mount_point?: string;
  overlay_modules?: string[];
  magic_modules?: string[];
  kasumi_modules?: string[];
  mount_error_modules?: string[];
  mount_error_reasons?: Record<string, string>;
  skip_mount_modules?: string[];
  blacklisted_modules?: string[];
  active_mounts?: string[];
  tmpfs_xattr_supported?: boolean;
  mode_stats?: RuntimeModeStatsPayload;
  kasumi?: RuntimeKasumiPayload;
  daemon?: { alive?: boolean; socket_path?: string; last_refresh_ts?: number };
  // Additional fields not yet typed:
  [key: string]: unknown;
}

export interface RuntimeModeStatsPayload {
  overlayfs?: number;
  magicmount?: number;
  kasumi?: number;
  blacklisted?: number;
}

export interface RuntimeKasumiPayload {
  status?: string;
  available?: boolean;
  lkm_loaded?: boolean;
  lkm_autoload?: boolean;
  lkm_kmi_override?: string;
  lkm_current_kmi?: string;
  lkm_dir?: string;
  protocol_version?: number | null;
  feature_bits?: number | null;
  feature_names?: string[];
  hooks?: string[];
  rule_count?: number;
  user_hide_rule_count?: number;
  mirror_path?: string;
}

import { PATHS } from "../../constants";
import { AppError } from "../core/error";
import { isRecord } from "../core/guards";
import { runDaemonCommand } from "../core/bridge";

export async function loadRuntimeState(): Promise<RuntimeStatePayload> {
  const direct = await runDaemonCommand({ type: "status" }, PATHS.BINARY);
  if (!isRecord(direct)) {
    throw new AppError("daemon status returned invalid payload");
  }
  return direct as RuntimeStatePayload;
}
