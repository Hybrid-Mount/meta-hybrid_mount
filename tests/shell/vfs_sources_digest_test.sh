#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

set -eu

TESTS_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(cd -- "$TESTS_DIR/../.." && pwd)
SCRIPT="$TESTS_DIR/vfs_sources_digest.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

fail() {
    echo "FAIL: $1" >&2
    exit 1
}

make_tree() {
    root=$1
    mkdir -p "$root/module/vfs/src" "$root/module/vfs/binaries"
    for f in hybridmount.c hybridmount.h Kconfig Makefile; do
        printf 'initial %s\n' "$f" > "$root/module/vfs/src/$f"
    done
    (
        cd "$root/module/vfs/src" &&
            sha256sum --text hybridmount.c hybridmount.h Kconfig Makefile |
            sha256sum | cut -d' ' -f1 > "$root/module/vfs/binaries/sources.txt"
    )
}

run() {
    REPO_ROOT=$1 sh "$SCRIPT" 2>&1
}

tree="$WORK/ok"
make_tree "$tree"
out=$(run "$tree") || fail "a matching tree must pass, got: $out"
case "$out" in
*PASS*) ;;
*) fail "expected PASS, got: $out" ;;
esac

tree="$WORK/drift"
make_tree "$tree"
printf 'a later fix\n' >> "$tree/module/vfs/src/hybridmount.c"
if out=$(run "$tree"); then
    fail "a source change must invalidate the prebuilt modules, got: $out"
fi
case "$out" in
*"do not match"*) ;;
*) fail "expected a mismatch message, got: $out" ;;
esac
case "$out" in
*"Build VFS kernel module"*) ;;
*) fail "the failure should say how to rebuild, got: $out" ;;
esac

tree="$WORK/missing"
make_tree "$tree"
rm "$tree/module/vfs/src/hybridmount.h"
if out=$(run "$tree"); then
    fail "a missing build input must fail, got: $out"
fi
case "$out" in
*"missing"*) ;;
*) fail "expected a missing-input message, got: $out" ;;
esac

tree="$WORK/badstamp"
make_tree "$tree"
printf 'not-a-digest\n' > "$tree/module/vfs/binaries/sources.txt"
if out=$(run "$tree"); then
    fail "a malformed stamp must fail, got: $out"
fi

# A missing stamp permits checks before the first prebuilt assembly.
tree="$WORK/nostamp"
make_tree "$tree"
rm "$tree/module/vfs/binaries/sources.txt"
out=$(run "$tree") || fail "a tree without prebuilt modules should skip: $out"
case "$out" in
*SKIP*) ;;
*) fail "expected SKIP, got: $out" ;;
esac

out=$(run "$REPO_ROOT") || fail "the repository is not self-consistent: $out"
case "$out" in
*PASS*) ;;
*) fail "expected the repository to PASS, got: $out" ;;
esac

echo "PASS: vfs sources digest checker accepts a match and rejects stale prebuilts"
