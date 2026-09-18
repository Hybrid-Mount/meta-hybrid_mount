#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Exercise tests/shell/vfs_sources_digest.sh: it must accept a matching tree and reject a
# prebuilt set whose sources have moved on. That drift is the failure it exists to catch, so
# a checker that only ever printed PASS would be worse than none at all.

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

# A miniature tree with the same shape the checker walks, so the test never depends on the
# real module sources staying byte-identical.
make_tree() {
    root=$1
    mkdir -p "$root/module/vfs/src" "$root/module/vfs/binaries"
    for f in hybridmount.c hybridmount.h Kconfig Makefile; do
        printf 'initial %s\n' "$f" > "$root/module/vfs/src/$f"
    done
    (
        cd "$root/module/vfs/src" &&
            sha256sum hybridmount.c hybridmount.h Kconfig Makefile |
            sha256sum | cut -d' ' -f1 > "$root/module/vfs/binaries/sources.txt"
    )
}

run() {
    REPO_ROOT=$1 sh "$SCRIPT" 2>&1
}

# 1. A freshly stamped tree passes.
tree="$WORK/ok"
make_tree "$tree"
out=$(run "$tree") || fail "a matching tree must pass, got: $out"
case "$out" in
*PASS*) ;;
*) fail "expected PASS, got: $out" ;;
esac

# 2. Changing a source file must be rejected: this is the shipped-stale-binary case.
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
# The advice must name the documented rebuild path.
case "$out" in
*"Build VFS kernel module"*) ;;
*) fail "the failure should say how to rebuild, got: $out" ;;
esac

# 3. A missing build input must be reported rather than silently dropped from the digest.
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

# 4. A malformed stamp must not be read as a match.
tree="$WORK/badstamp"
make_tree "$tree"
printf 'not-a-digest\n' > "$tree/module/vfs/binaries/sources.txt"
if out=$(run "$tree"); then
    fail "a malformed stamp must fail, got: $out"
fi

# 5. With no stamp at all the check skips, so the workflow can run before the first assembly.
tree="$WORK/nostamp"
make_tree "$tree"
rm "$tree/module/vfs/binaries/sources.txt"
out=$(run "$tree") || fail "a tree without prebuilt modules should skip: $out"
case "$out" in
*SKIP*) ;;
*) fail "expected SKIP, got: $out" ;;
esac

# 6. The real repository must currently be consistent.
out=$(run "$REPO_ROOT") || fail "the repository is not self-consistent: $out"
case "$out" in
*PASS*) ;;
*) fail "expected the repository to PASS, got: $out" ;;
esac

echo "PASS: vfs sources digest checker accepts a match and rejects stale prebuilts"
