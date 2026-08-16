#!/system/bin/sh
# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-only

############################################
# mix-mount uninstall.sh
# Cleanup script for metamodule removal
############################################

MODDIR="${0%/*}"
BASE_DIR="/data/adb/hybrid-mount"
BINARY="$MODDIR/hybrid-mount"

# Best-effort teardown: stop the resident daemon first, then ask the binary to
# detach mounts that a previous Hybrid Mount run can attribute to itself.
# The Nano flavor is mount-only and has no CLI, so skip both for it.
if [ -f "$MODDIR/.nano" ]; then
  :
elif [ -x "$BINARY" ]; then
  "$BINARY" daemon stop >/dev/null 2>&1 || true
  "$BINARY" emulated-soft-reboot >/dev/null 2>&1 || true
fi

# Fallback for a daemon that ignored or predated the stop request.
if [ -r "$BASE_DIR/run/daemon.pid" ]; then
  DAEMON_PID="$(cat "$BASE_DIR/run/daemon.pid" 2>/dev/null)" || DAEMON_PID=""
  case "$DAEMON_PID" in
  '' | *[!0-9]*)
    ;;
  *)
    kill "$DAEMON_PID" >/dev/null 2>&1 || true
    ;;
  esac
fi

rm -rf "$BASE_DIR"

exit 0
