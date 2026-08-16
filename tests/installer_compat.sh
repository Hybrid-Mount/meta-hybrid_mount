#!/bin/sh
# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

set -eu

SCRIPT_DIR="$(CDPATH='' cd "$(dirname "$0")" && pwd)"
# shellcheck disable=SC1091
. "$SCRIPT_DIR/../module/install_compat.sh"

TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/hybrid-mount-installer-compat.XXXXXX")"
trap 'rm -rf "$TEST_ROOT"' EXIT HUP INT TERM

ACTIVE_MODULE_DIR="$TEST_ROOT/active"
PACKAGE_DIR="$TEST_ROOT/package"
mkdir -p "$PACKAGE_DIR"
printf 'id=hybrid_mount\nupgradeEpoch=1\n' >"$PACKAGE_DIR/module.prop"

fail() {
  printf 'installer compatibility test failed: %s\n' "$1" >&2
  exit 1
}

assert_requires_reinstall() {
  if ! hybrid_mount_requires_clean_reinstall "$ACTIVE_MODULE_DIR" "$REQUIRED_EPOCH"; then
    fail "$1"
  fi
}

assert_allows_install() {
  if hybrid_mount_requires_clean_reinstall "$ACTIVE_MODULE_DIR" "$REQUIRED_EPOCH"; then
    fail "$1"
  fi
}

assert_pending_state() {
  module_dir="$1"
  count="$(grep -c '^upgradeState=clean-reinstall-required$' "$module_dir/module.prop" || true)"
  [ "$count" = "1" ] || fail "$2"
}

REQUIRED_EPOCH="$(hybrid_mount_read_upgrade_epoch "$PACKAGE_DIR")" || fail "package marker is invalid"
[ "$REQUIRED_EPOCH" = "1" ] || fail "unexpected package epoch"

assert_allows_install "a missing active module must be treated as a fresh install"

mkdir -p "$ACTIVE_MODULE_DIR"
assert_allows_install "a directory without module.prop must not be treated as an installation"

: >"$ACTIVE_MODULE_DIR/module.prop"
assert_requires_reinstall "a pre-mechanism installation without an epoch must require a clean reinstall"

printf 'upgradeEpoch=invalid\n' >"$ACTIVE_MODULE_DIR/module.prop"
assert_requires_reinstall "an invalid installed epoch must be rejected"

printf 'upgradeEpoch=0\n' >"$ACTIVE_MODULE_DIR/module.prop"
assert_requires_reinstall "an older upgrade epoch must be rejected"

printf 'upgradeEpoch=%s\n' "$REQUIRED_EPOCH" >"$ACTIVE_MODULE_DIR/module.prop"
assert_allows_install "the current upgrade epoch must allow an in-place or pending upgrade"

printf 'upgradeEpoch=%s\nupgradeState=clean-reinstall-required\n' \
  "$REQUIRED_EPOCH" >"$ACTIVE_MODULE_DIR/module.prop"
assert_requires_reinstall "a pending state must remain sticky across same-epoch installs"

printf 'upgradeEpoch=2\n' >"$ACTIVE_MODULE_DIR/module.prop"
assert_requires_reinstall "a different upgrade epoch must be rejected"

printf 'id=hybrid_mount\ndescription=Hybrid Mount test\nupgradeEpoch=1\n' >"$PACKAGE_DIR/module.prop"
hybrid_mount_mark_clean_reinstall_required "$PACKAGE_DIR" || fail "failed to mark package state"
assert_pending_state "$PACKAGE_DIR" "the staged package must record a pending clean reinstall"
grep -q '^description=\[Action required\] Reboot, then open WebUI to complete a clean reinstall\.$' \
  "$PACKAGE_DIR/module.prop" || fail "the module description must expose the required action"

hybrid_mount_mark_clean_reinstall_required "$PACKAGE_DIR" || fail "failed to refresh package state"
assert_pending_state "$PACKAGE_DIR" "marking twice must not duplicate the pending state"

hybrid_mount_mark_clean_reinstall_required \
  "$PACKAGE_DIR" \
  "[Action required] Uninstall Hybrid Mount, reboot, then reinstall this ZIP." || \
  fail "failed to set a flavor-specific description"
grep -q '^description=\[Action required\] Uninstall Hybrid Mount, reboot, then reinstall this ZIP\.$' \
  "$PACKAGE_DIR/module.prop" || fail "the Nano description must not refer to WebUI"
