#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

# metamount.sh — 启动期入口:调用唯一二进制执行完整挂载流水线。

MODDIR="${0%/*}"
BASE_DIR="/data/adb/hybrid-mount"
RUN_DIR="$BASE_DIR/run"

mkdir -p "$BASE_DIR" "$RUN_DIR"

BINARY="$MODDIR/hybrid-mount"
LOCK_DIR="/dev/hybrid_mount_single_instance"

if ! mkdir "$LOCK_DIR" 2>/dev/null; then
  echo "Hybrid Mount already ran during this boot"
  exit 0
fi

cleanup_lock() {
  rmdir "$LOCK_DIR" 2>/dev/null
}

if [ ! -f "$BINARY" ]; then
  echo "ERROR: Binary not found at $BINARY"
  cleanup_lock
  exit 1
fi

chmod 755 "$BINARY"

"$BINARY"
STATUS=$?

if [ "$STATUS" -eq 0 ] && [ -x /data/adb/ksud ]; then
  /data/adb/ksud kernel notify-module-mounted
fi

exit "$STATUS"
