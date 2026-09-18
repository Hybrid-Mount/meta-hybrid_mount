#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# The prebuilt hybridmount modules ship inside the release zip, so a .ko that no longer
# matches the kernel sources beside it is a shipped defect: users get behaviour the source
# in the same package does not describe. Nothing about the binary digests in list.txt can
# reveal that, because they only prove the files are unchanged since they were built.
#
# `.github/workflows/kernel-module.yml` records a digest of the exact build inputs next to
# the modules whenever it compiles them; this script recomputes that digest and fails when
# the two disagree. It runs in that workflow right after assembly, and again in lints.yml on
# every change, so a drift introduced by any other route is caught too.
#
# Exit status is also used by tests/shell/vfs_sources_digest_test.sh, so select the tree by
# REPO_ROOT rather than assuming the caller's working directory.

set -eu

REPO_ROOT=${REPO_ROOT:-$(cd -- "$(dirname -- "$0")/../.." && pwd)}
SRC_DIR="$REPO_ROOT/module/vfs/src"
SOURCES_STAMP="$REPO_ROOT/module/vfs/binaries/sources.txt"

# The build inputs, in the fixed order the workflow digests them. Making this list explicit
# means adding a source file to the module is a visible decision rather than a silent gap.
# `set --` turns the fixed, space-separated list into positional parameters, so sha256sum
# receives four separate names that shellcheck can see without a suppression.
BUILD_INPUTS="hybridmount.c hybridmount.h Kconfig Makefile"
# shellcheck disable=SC2086 # splitting the fixed literal list above is the point
set -- $BUILD_INPUTS

fail() {
    echo "FAIL: $1" >&2
    exit 1
}

[ -d "$SRC_DIR" ] || fail "missing $SRC_DIR"
if [ ! -f "$SOURCES_STAMP" ]; then
    # No prebuilt modules are committed yet, so there is no compiled artefact to contradict
    # the sources. The workflow creates the stamp the first time it assembles them.
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
        sha256sum "$@" | sha256sum | cut -d' ' -f1
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
