#!/system/bin/sh
# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-only

CONFIG_FILE="/data/adb/hybrid-mount/config.toml"
if [ -f "$CONFIG_FILE" ]; then
  MODE=$(grep -E '^[[:space:]]*daemon_startup_mode[[:space:]]*=' "$CONFIG_FILE" | sed 's/.*=[[:space:]]*"\(.*\)".*/\1/')
  if [ "$MODE" = "persistent" ]; then
    /data/adb/modules/hybrid_mount/hybrid-mount daemon serve &
  fi
fi
