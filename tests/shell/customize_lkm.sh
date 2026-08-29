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
  case_dir=$(mktemp -d)
  mkdir "$case_dir/lkm"

  (
    MODPATH="$case_dir"
    ARCH="arm64"
    KSU="$ksu_value"
    APATCH="$apatch_value"
    export MODPATH ARCH KSU APATCH

    ui_print() { :; }
    abort() { exit 0; }

    # The missing userspace binary stops customize.sh immediately after the
    # platform-dependent LKM retention branch.
    . "$ROOT_DIR/module/customize.sh"
  )

  if [ "$expect_lkm" = "yes" ]; then
    test -d "$case_dir/lkm"
    rmdir "$case_dir/lkm"
  else
    test ! -e "$case_dir/lkm"
  fi
  rmdir "$case_dir"
  printf '%s installer branch: ok\n' "$case_name"
}

run_case "KernelSU" "true" "" "no"
run_case "APatch" "" "true" "yes"
