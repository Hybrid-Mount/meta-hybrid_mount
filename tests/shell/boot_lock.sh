#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)"
WORK=$(mktemp -d)
trap 'rm -r "$WORK"' EXIT HUP INT TERM
cp "$ROOT/module/metamount.sh" "$WORK/"
cat > "$WORK/hybrid-mount" <<'BIN'
#!/bin/sh
[ "$*" = boot ] || exit 99
exit "${TEST_EXIT:-0}"
BIN
chmod +x "$WORK/hybrid-mount"
sh "$WORK/metamount.sh"
if TEST_EXIT=42 sh "$WORK/metamount.sh"; then
  echo 'FAIL: boot failure swallowed' >&2
  exit 1
fi
printf '%s\n' 'PASS: mount hook delegates process locking and preserves boot failures'
