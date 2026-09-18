#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# metamount.sh takes the single-instance lock and boot-completed.sh releases it. Both
# scripts run as separate processes with no shared shell state, so the path cannot be
# factored into a sourced file without adding a boot-time failure mode. Pin the coupling
# with a check instead: if these two ever disagree, the lock is either never taken or
# never released, and the second failure silently disables the post-boot hot-install run.

set -eu

ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)"
META="$ROOT/module/metamount.sh"
BOOT="$ROOT/module/boot-completed.sh"

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

for script in "$META" "$BOOT"; do
  [ -f "$script" ] || fail "missing $script"
done

# Extract the literal assigned to LOCK_DIR.
lock_of() {
  sed -n 's/^[[:space:]]*LOCK_DIR="\([^"]*\)".*$/\1/p' "$1" | head -n 1
}

meta_lock="$(lock_of "$META")"
boot_lock="$(lock_of "$BOOT")"

[ -n "$meta_lock" ] || fail "metamount.sh does not assign LOCK_DIR"
[ -n "$boot_lock" ] || fail "boot-completed.sh does not assign LOCK_DIR"
[ "$meta_lock" = "$boot_lock" ] ||
  fail "LOCK_DIR differs: metamount.sh='$meta_lock' boot-completed.sh='$boot_lock'"

case "$meta_lock" in
  /*) ;;
  *) fail "LOCK_DIR must be absolute: '$meta_lock'" ;;
esac

# metamount.sh must create the lock before running the binary and must release it on the
# binary-missing path, otherwise a failed boot leaves every later boot locked out.
# shellcheck disable=SC2016 # matching the literal source text, not expanding it
grep -q 'mkdir "$LOCK_DIR"' "$META" || fail "metamount.sh no longer creates the lock"
# shellcheck disable=SC2016
grep -q 'rmdir "$LOCK_DIR"' "$META" || fail "metamount.sh no longer cleans up the lock"
# shellcheck disable=SC2016
grep -q 'rmdir "$LOCK_DIR"' "$BOOT" || fail "boot-completed.sh no longer releases the lock"

echo "PASS: boot lock path and lifecycle are consistent ($meta_lock)"
