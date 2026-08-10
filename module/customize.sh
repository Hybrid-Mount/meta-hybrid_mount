# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-only
# shellcheck shell=sh disable=SC3043

if [ -z "$APATCH" ] && [ -z "$KSU" ]; then
  abort "! unsupported root platform"
fi

unzip -o "$ZIPFILE" -d "$MODPATH" >&2
case "$ARCH" in
"arm64")
  ;;
*)
  abort "! Unsupported architecture: $ARCH (Hybrid Mount now supports arm64 only)"
  ;;
esac
ui_print "- Device Architecture: $ARCH"

NANO_MODE=false
if [ -f "$MODPATH/.nano" ]; then
  NANO_MODE=true
  ui_print "- Flavor: Nano (config-only)"
fi

INSTALL_COMPAT_HELPER="$MODPATH/install_compat.sh"
ACTIVE_MODULE_DIR="/data/adb/modules/hybrid_mount"
CLEAN_REINSTALL_REQUIRED=false
if [ ! -f "$INSTALL_COMPAT_HELPER" ]; then
  abort "! Missing installer compatibility helper"
fi
# shellcheck disable=SC1090,SC1091
. "$INSTALL_COMPAT_HELPER"

REQUIRED_UPGRADE_EPOCH="$(hybrid_mount_read_upgrade_epoch "$MODPATH")" || abort "! Invalid upgrade compatibility marker"
if hybrid_mount_requires_clean_reinstall "$ACTIVE_MODULE_DIR" "$REQUIRED_UPGRADE_EPOCH"; then
  if [ "$NANO_MODE" = "true" ]; then
    REINSTALL_DESCRIPTION="[Action required] Uninstall Hybrid Mount, reboot, then reinstall this ZIP."
  else
    REINSTALL_DESCRIPTION="$HYBRID_MOUNT_REINSTALL_DESCRIPTION"
  fi
  hybrid_mount_mark_clean_reinstall_required "$MODPATH" "$REINSTALL_DESCRIPTION" || abort "! Failed to record clean reinstall state"
  CLEAN_REINSTALL_REQUIRED=true
  ui_print " "
  ui_print "========================================"
  ui_print "! Clean reinstall required"
  ui_print "! The installed version either predates this"
  ui_print "! compatibility mechanism or uses a different"
  ui_print "! upgrade epoch. It cannot be upgraded in place."
  if [ "$NANO_MODE" = "true" ]; then
    ui_print "! Nano has no WebUI. Uninstall Hybrid Mount in"
    ui_print "! KernelSU/APatch, reboot, then install again."
  else
    ui_print "! Reboot once, then open WebUI for guided steps."
  fi
  ui_print "! After reboot, Hybrid Mount stays paused until completed."
  ui_print "========================================"
fi
rm -f "$INSTALL_COMPAT_HELPER"

BIN_SOURCE="$MODPATH/binaries/hybrid-mount"
BIN_TARGET="$MODPATH/hybrid-mount"
if [ ! -f "$BIN_SOURCE" ]; then
  abort "! Binary not found in this zip!"
fi
ui_print "- Installing binary..."
cp -f "$BIN_SOURCE" "$BIN_TARGET"
set_perm "$BIN_TARGET" 0 0 0755
rm -rf "$MODPATH/binaries"
rm -rf "$MODPATH/system"
if [ "$NANO_MODE" = "true" ]; then
  rm -rf "$MODPATH/webroot" "$MODPATH/launcher.png"
fi
BASE_DIR="/data/adb/hybrid-mount"

wait_volume_key_or_timeout() {
  local timeout_seconds start_time current_time key_event
  timeout_seconds=$1
  start_time=$(date +%s)
  while true; do
    current_time=$(date +%s)
    if [ $((current_time - start_time)) -ge "$timeout_seconds" ]; then
      printf 'timeout\n'
      return 0
    fi
    key_event=$(timeout 0.5 getevent -l 2>/dev/null)
    if echo "$key_event" | grep -q "KEY_VOLUMEUP"; then
      printf 'up\n'
      return 0
    elif echo "$key_event" | grep -q "KEY_VOLUMEDOWN"; then
      printf 'down\n'
      return 0
    fi
  done
}

KEY_volume_detect() {
  ui_print " "
  ui_print "========================================"
  ui_print "      Select Default Mount Mode      "
  ui_print "========================================"
  ui_print "  Volume Up (+): OverlayFS"
  ui_print "  Volume Down (-): Magic Mount"
  ui_print " "
  ui_print "  Defaulting to OverlayFS in 10 seconds"
  ui_print "========================================"
  local timeout=10
  local chosen_mode="overlay"
  case "$(wait_volume_key_or_timeout "$timeout")" in
  up)
    chosen_mode="overlay"
    ui_print "- Key Detected: Selected OverlayFS"
    ;;
  down)
    chosen_mode="magic"
    ui_print "- Key Detected: Selected Magic Mount"
    ;;
  timeout)
    ui_print "- Timeout: Selected OverlayFS"
    ;;
  esac
  ui_print "- Configured mode: $chosen_mode"
  sed -i "s/^default_mode = .*/default_mode = \"$chosen_mode\"/" "$BASE_DIR/config.toml"
}

if [ "$CLEAN_REINSTALL_REQUIRED" = "true" ]; then
  ui_print "- Skipping configuration changes in maintenance mode"
else
  mkdir -p "$BASE_DIR"

  if [ ! -f "$BASE_DIR/config.toml" ]; then
    ui_print "- Fresh installation detected"
    ui_print "- Installing default config..."
    cat "$MODPATH/config.toml" >"$BASE_DIR/config.toml"
    if [ "$NANO_MODE" = "true" ]; then
      ui_print "- Nano mode uses config.toml only; skipping setup wizard"
    else
      KEY_volume_detect
    fi
  else
    ui_print "- Existing config found"
    ui_print "- Skipping setup wizard to preserve settings"
  fi

  if [ ! -f "$BASE_DIR/module_blacklist.toml" ]; then
    ui_print "- Installing default module blacklist..."
    cat "$MODPATH/module_blacklist.toml" >"$BASE_DIR/module_blacklist.toml"
  fi
fi

set_perm_recursive "$MODPATH" 0 0 0755 0644
set_perm "$BIN_TARGET" 0 0 0755
if [ "$CLEAN_REINSTALL_REQUIRED" = "true" ]; then
  ui_print "- Installation complete (clean reinstall still required)"
else
  ui_print "- Installation complete"
fi
