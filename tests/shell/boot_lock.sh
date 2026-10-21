#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)"
WORK=$(mktemp -d)
trap 'rm -r "$WORK"' EXIT HUP INT TERM
cp "$ROOT/module/metamount.sh" "$ROOT/module/post-mount.sh" "$WORK/"
cat > "$WORK/hybrid-mount" <<'BIN'
#!/bin/sh
[ "$*" = "boot $TEST_STAGE" ] || exit 99
exit "${TEST_EXIT:-0}"
BIN
chmod +x "$WORK/hybrid-mount"
for stage in metamount post-mount; do
  TEST_STAGE="$stage" sh "$WORK/$stage.sh"
  if TEST_STAGE="$stage" TEST_EXIT=42 sh "$WORK/$stage.sh"; then
    echo "FAIL: $stage swallowed the boot failure" >&2
    exit 1
  else
    status=$?
    [ "$status" -eq 42 ]
  fi
done
printf '%s\n' 'PASS: boot hooks forward their stage and preserve failures'
