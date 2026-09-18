#!/bin/sh
# shellcheck disable=SC1090
# SPDX-License-Identifier: GPL-3.0-only

set -eu

ROOT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)

run_case() {
  case_name="$1"
  ksu_value="$2"
  apatch_value="$3"
  expect_lkm="$4"
  arch_value="${5:-arm64}"
  case_dir=$(mktemp -d)
  mkdir "$case_dir/lkm"
  mkdir -p "$case_dir/vfs/src"
  : > "$case_dir/vfs/src/hybridmount.c"

  (
    MODPATH="$case_dir"
    ARCH="$arch_value"
    KSU="$ksu_value"
    APATCH="$apatch_value"
    export MODPATH ARCH KSU APATCH

    ui_print() { :; }
    abort() { exit 0; }

    # The missing userspace binary stops customize.sh immediately after the
    # platform-dependent retention branches.
    . "$ROOT_DIR/module/customize.sh"
  )

  if [ "$expect_lkm" = "yes" ]; then
    test -d "$case_dir/lkm"
    rmdir "$case_dir/lkm"
  else
    test ! -e "$case_dir/lkm"
  fi
  # K2 sources remain available for built-in kernel integration on every platform.
  test -f "$case_dir/vfs/src/hybridmount.c" ||
    { echo "FAIL: $case_name dropped vfs/src" >&2; exit 1; }
  rm -rf "$case_dir/vfs/src"
  rmdir "$case_dir/vfs"
  rmdir "$case_dir"
  printf '%s installer branch: ok\n' "$case_name"
}

run_case "KernelSU arm64" "true" "" "no"
run_case "APatch arm64" "" "true" "yes"
run_case "APatch armv7" "" "true" "yes" "arm"
