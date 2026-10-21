#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

set -eu

REPO_ROOT=${REPO_ROOT:-$(cd -- "$(dirname -- "$0")/../.." && pwd)}
SRC_DIR="$REPO_ROOT/module/vfs/src"
SOURCES_STAMP="$REPO_ROOT/module/vfs/binaries/sources.txt"

# receives four separate names that shellcheck can see without a suppression.
BUILD_INPUTS="hybridmount.c hybridmount.h Kconfig Makefile"
# shellcheck disable=SC2086
set -- $BUILD_INPUTS

fail() {
    echo "FAIL: $1" >&2
    exit 1
}

[ -d "$SRC_DIR" ] || fail "missing $SRC_DIR"
if [ ! -f "$SOURCES_STAMP" ]; then
    echo "SKIP: no $SOURCES_STAMP; the prebuilt modules have not been assembled yet"
    exit 0
fi

recorded=$(tr -d '[:space:]' < "$SOURCES_STAMP")
case "$recorded" in
'' | *[!0-9a-f]*)
    fail "$SOURCES_STAMP does not contain a single sha256 digest"
    ;;
esac
[ "${#recorded}" -eq 64 ] || fail "$SOURCES_STAMP must hold a 64-character sha256 digest"

for input in "$@"; do
    [ -f "$SRC_DIR/$input" ] || fail "build input $SRC_DIR/$input is missing"
done

actual=$(
    cd "$SRC_DIR" &&
        sha256sum --text "$@" | sha256sum | cut -d' ' -f1
)

if [ "$recorded" != "$actual" ]; then
    echo "FAIL: the prebuilt hybridmount modules do not match module/vfs/src" >&2
    echo "  recorded by the build: $recorded" >&2
    echo "  current sources:       $actual" >&2
    echo "Rebuild and commit them with:" >&2
    echo "  gh workflow run 'Build VFS kernel module' --ref dev -f commit_binaries=true" >&2
    exit 1
fi

echo "PASS: prebuilt hybridmount modules match the recorded source digest"
