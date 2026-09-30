#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# The Rust boot entry owns process locking and generation-aware deduplication.
MODDIR="${0%/*}"
BINARY="$MODDIR/hybrid-mount"
if [ ! -x "$BINARY" ]; then
  echo "ERROR: Binary not executable: $BINARY" >&2
  exit 1
fi
"$BINARY" boot
STATUS=$?
if [ "$STATUS" -eq 0 ] && [ -x /data/adb/ksud ]; then
  /data/adb/ksud kernel notify-module-mounted
fi
exit "$STATUS"
