/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */


export const APP_VERSION = "v0.0.0-mock";
export const RUST_PATHS = {
  CONFIG: "/data/adb/meta-hybrid/config.toml",
  MODE_CONFIG: "/data/adb/meta-hybrid/module_mode.conf",
  IMAGE_MNT: "/data/adb/meta-hybrid/mnt",
  DAEMON_STATE: "/data/adb/meta-hybrid/run/daemon_state.json",
  DAEMON_LOG: "/data/adb/meta-hybrid/daemon.log",
} as const;
export const BUILTIN_PARTITIONS = ["system", "vendor", "product", "system_ext", "odm", "oem", "apex"] as const;