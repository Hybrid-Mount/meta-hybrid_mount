// SPDX-License-Identifier: Apache-2.0

import type { AppConfig } from "./types";

export const MODULE_ID = import.meta.env.MODULE_ID;

export const PATHS = {
  BINARY: `/data/adb/modules/${MODULE_ID}/hybrid-mount`,
};

export const DEFAULT_CONFIG: AppConfig = {
  moduledir: "/data/adb/modules",
  mountsource: "KSU",
  overlay_mode: "ext4",
  disable_umount: false,
  default_mode: "overlay",
  rules: {},
};

export const MOUNT_MODES = ["overlay", "magic", "ignore"] as const;

export const BUILTIN_PARTITIONS = ["vendor", "system_ext", "product", "odm"] as const;

export const REPOSITORY_URL = "https://github.com/Hybrid-Mount/meta-hybrid_mount";
export const TELEGRAM_URL = "https://t.me/hybridmountchat";
