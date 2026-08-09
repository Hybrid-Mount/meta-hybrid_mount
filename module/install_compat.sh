# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-only
# shellcheck shell=sh disable=SC3043

HYBRID_MOUNT_UPGRADE_EPOCH_PROP="upgradeEpoch"
HYBRID_MOUNT_UPGRADE_STATE_PROP="upgradeState"
HYBRID_MOUNT_CLEAN_REINSTALL_STATE="clean-reinstall-required"
HYBRID_MOUNT_REINSTALL_DESCRIPTION="[Action required] Reboot, then open WebUI to complete a clean reinstall."

hybrid_mount_read_upgrade_epoch() {
  local module_dir epoch
  module_dir="$1"

  if [ ! -f "$module_dir/module.prop" ]; then
    return 1
  fi

  epoch="$(sed -n "s/^${HYBRID_MOUNT_UPGRADE_EPOCH_PROP}=//p" "$module_dir/module.prop" | head -n 1)"
  case "$epoch" in
  "" | *[!0-9]*)
    return 1
    ;;
  esac

  printf '%s\n' "$epoch"
}

hybrid_mount_requires_clean_reinstall() {
  local installed_module_dir required_epoch installed_epoch
  installed_module_dir="$1"
  required_epoch="$2"

  if [ ! -f "$installed_module_dir/module.prop" ]; then
    return 1
  fi

  if grep -q "^${HYBRID_MOUNT_UPGRADE_STATE_PROP}=" "$installed_module_dir/module.prop"; then
    return 0
  fi

  # An installed module.prop without a valid epoch predates this mechanism.
  installed_epoch="$(hybrid_mount_read_upgrade_epoch "$installed_module_dir")" || return 0
  [ "$installed_epoch" != "$required_epoch" ]
}

hybrid_mount_mark_clean_reinstall_required() {
  local module_dir module_prop temporary_prop reinstall_description
  module_dir="$1"
  module_prop="$module_dir/module.prop"
  temporary_prop="$module_prop.upgrade-state.$$"
  reinstall_description="${2:-$HYBRID_MOUNT_REINSTALL_DESCRIPTION}"

  if [ ! -f "$module_prop" ]; then
    return 1
  fi

  if ! sed \
    -e "/^${HYBRID_MOUNT_UPGRADE_STATE_PROP}=/d" \
    -e "s|^description=.*|description=${reinstall_description}|" \
    "$module_prop" >"$temporary_prop"; then
    rm -f "$temporary_prop"
    return 1
  fi
  printf '%s=%s\n' \
    "$HYBRID_MOUNT_UPGRADE_STATE_PROP" \
    "$HYBRID_MOUNT_CLEAN_REINSTALL_STATE" >>"$temporary_prop" || {
    rm -f "$temporary_prop"
    return 1
  }
  cat "$temporary_prop" >"$module_prop" || {
    rm -f "$temporary_prop"
    return 1
  }
  rm -f "$temporary_prop"
}
