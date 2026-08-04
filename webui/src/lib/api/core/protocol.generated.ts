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

// Generated from the non-Kasumi commands in src/core/daemon/protocol.rs.
// Do not edit by hand.

export const DAEMON_COMMAND_TYPES = [
  "api-config-get",
  "api-config-patch",
  "api-config-reset",
  "api-config-set",
  "api-kernel-uname",
  "api-modules-apply",
  "api-modules-list",
  "api-mount-stats",
  "api-mount-topology",
  "api-open-url",
  "api-partitions",
  "api-reboot",
  "api-storage",
  "api-system-info",
  "api-version",
  "init",
  "ping",
  "shutdown",
  "status",
  "webui-start",
] as const;

export type DaemonCommandType = (typeof DAEMON_COMMAND_TYPES)[number];
