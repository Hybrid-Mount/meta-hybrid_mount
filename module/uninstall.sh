#!/system/bin/sh
# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

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
