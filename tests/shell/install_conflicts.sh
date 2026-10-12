#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
WORK=$(mktemp -d)
trap 'rm -r "$WORK"' EXIT HUP INT TERM
mkdir -p "$WORK/binaries" "$WORK/lkm" "$WORK/vfs/binaries"
cat > "$WORK/binaries/hybrid-mount-arm64" <<'BIN'
#!/bin/sh
[ "$*" = check-conflicts ] || exit 99
echo 'susfs_guard_lkm.ko detected' >&2
exit 42
BIN
chmod +x "$WORK/binaries/hybrid-mount-arm64"
if (
  MODPATH=$WORK ARCH=arm64 KSU=true APATCH=''
  ui_print() { :; }
  abort() { echo "$*" >&2; exit 1; }
  set_perm() { :; }
  # Keep a broken installer from writing the host's /data/adb during this test.
  mkdir() { echo 'FAIL: installer reached persistent config creation' >&2; exit 98; }
  . "$ROOT/module/customize.sh"
) > "$WORK/output" 2>&1; then
  echo 'FAIL: conflicting installer succeeded' >&2
  exit 1
fi
grep -q 'susfs_guard_lkm' "$WORK/output"
test -d "$WORK/lkm"
test -f "$WORK/binaries/hybrid-mount-arm64"
test ! -e "$WORK/hybrid-mount"
printf '%s\n' 'PASS: installer rejects conflict before modifying the package or runtime config'
