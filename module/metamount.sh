# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-only

# shellcheck shell=sh

MODDIR="${0%/*}"
BASE_DIR="/data/adb/hybrid-mount"
RUN_DIR="$BASE_DIR/run"
PID_FILE="$RUN_DIR/daemon.pid"
SOCKET_FILE="$RUN_DIR/daemon.sock"
STATE_FILE="$RUN_DIR/daemon_state.json"

cleanup_runtime_files() {
  rm -f "$PID_FILE" "$SOCKET_FILE" "$STATE_FILE"
}

if [ -f "$MODDIR/module.prop" ] && grep -q '^upgradeState=' "$MODDIR/module.prop"; then
  cleanup_runtime_files
  echo "WARN: Hybrid Mount is paused until a clean reinstall is completed"
  if [ -x /data/adb/ksud ]; then
    /data/adb/ksud kernel notify-module-mounted
  fi
  exit 0
fi

mkdir -p "$BASE_DIR" "$RUN_DIR"

BINARY="$MODDIR/hybrid-mount"

if [ ! -f "$BINARY" ]; then
  echo "ERROR: Binary not found at $BINARY"
  exit 1
fi

chmod 755 "$BINARY"
cleanup_runtime_files

"$BINARY"
STATUS=$?

if [ "$STATUS" -eq 0 ] && [ -x /data/adb/ksud ]; then
  /data/adb/ksud kernel notify-module-mounted
fi

exit "$STATUS"
