#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)"
WORK=$(mktemp -d)
trap 'rm -r "$WORK"' EXIT HUP INT TERM
cp "$ROOT/module/emulated-soft-reboot.sh" "$WORK/"
cat > "$WORK/hybrid-mount" <<'BIN'
#!/bin/sh
[ "$*" = "runtime prepare-reboot" ] || exit 99
exit "${TEST_EXIT:-0}"
BIN
chmod +x "$WORK/hybrid-mount"
sh "$WORK/emulated-soft-reboot.sh"
if TEST_EXIT=42 sh "$WORK/emulated-soft-reboot.sh"; then
  echo 'FAIL: cleanup error swallowed' >&2
  exit 1
fi
printf '%s\n' 'PASS: soft-reboot hook delegates cleanup and preserves failures'
