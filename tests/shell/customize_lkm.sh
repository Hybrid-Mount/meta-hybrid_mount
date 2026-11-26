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
  expect_vfs="${6:-yes}"
  case_dir=$(mktemp -d)
  printf 'packageFlavor=normal\n' > "$case_dir/module.prop"
  mkdir "$case_dir/lkm"
  mkdir -p "$case_dir/vfs/src" "$case_dir/vfs/binaries"
  : > "$case_dir/vfs/src/hybridmount.c"

  (
    MODPATH="$case_dir"
    ARCH="$arch_value"
    KSU="$ksu_value"
    APATCH="$apatch_value"
    export MODPATH ARCH KSU APATCH

    ui_print() { :; }
    abort() { exit 0; }

    . "$ROOT_DIR/module/customize.sh"
  )

  if [ "$expect_lkm" = "yes" ]; then
    test -d "$case_dir/lkm"
    rmdir "$case_dir/lkm"
  else
    test ! -e "$case_dir/lkm"
  fi
  if [ "$expect_vfs" = "yes" ]; then
    test -d "$case_dir/vfs/binaries"
    rmdir "$case_dir/vfs/binaries"
  else
    test ! -e "$case_dir/vfs/binaries"
  fi
  test -f "$case_dir/vfs/src/hybridmount.c" ||
    { echo "FAIL: $case_name dropped vfs/src" >&2; exit 1; }
  rm -rf "$case_dir/vfs/src"
  rmdir "$case_dir/vfs"
  rm "$case_dir/module.prop"
  rmdir "$case_dir"
  printf '%s installer branch: ok\n' "$case_name"
}

run_case "KernelSU arm64" "true" "" "no"
run_case "APatch arm64" "" "true" "yes"
run_case "APatch armv7" "" "true" "yes" "arm" "no"
run_case "APatch riscv64" "" "true" "yes" "riscv64" "no"

for wear_arch in arm64 arm x64 riscv64; do
  case_dir=$(mktemp -d)
  printf 'packageFlavor=wear\n' > "$case_dir/module.prop"
  message=$(
    MODPATH="$case_dir"
    ARCH="$wear_arch"
    KSU=true
    APATCH=
    ui_print() { :; }
    abort() { printf '%s\n' "$*"; exit 0; }
    . "$ROOT_DIR/module/customize.sh"
  )
  case "$wear_arch:$message" in
  arm64:*Binary\ not\ found* | arm:*Binary\ not\ found* | x64:*Binary\ not\ found*) ;;
  riscv64:*Wear\ package\ supports*) ;;
  *) echo "FAIL: Wear architecture gate: $wear_arch: $message" >&2; exit 1 ;;
  esac
  rm "$case_dir/module.prop"
  rmdir "$case_dir"
done
printf 'Wear installer architecture gate: ok\n'
