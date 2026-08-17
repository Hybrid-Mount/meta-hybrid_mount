#!/system/bin/sh
# shellcheck disable=SC3043
# ReHybrid-Mount
#
# SPDX-License-Identifier: GPL-3.0-only

# customize.sh — 安装阶段:平台检查、二进制落位、默认配置与初始化向导。

if [ -z "$APATCH" ] && [ -z "$KSU" ]; then
  abort "! unsupported root platform"
fi

case "$ARCH" in
arm64)
  BIN_FILE="hybrid-mount-arm64"
  ;;
arm)
  BIN_FILE="hybrid-mount-armv7"
  ;;
x64)
  BIN_FILE="hybrid-mount-x86_64"
  ;;
*)
  abort "! Unsupported architecture: $ARCH (supported: arm64, armv7, x86_64)"
  ;;
esac

ui_print "- Device Architecture: $ARCH"

BIN_SOURCE="$MODPATH/binaries/$BIN_FILE"
BIN_TARGET="$MODPATH/hybrid-mount"
if [ ! -f "$BIN_SOURCE" ]; then
  abort "! Binary not found in this zip!"
fi

ui_print "- Installing binary..."
cp -f "$BIN_SOURCE" "$BIN_TARGET"
set_perm "$BIN_TARGET" 0 0 0755
rm -rf "$MODPATH/binaries"

BASE_DIR="/data/adb/hybrid-mount"
mkdir -p "$BASE_DIR"

wait_volume_key_or_timeout() {
  local timeout_seconds="$1"
  local start_time
  start_time=$(date +%s)
  while true; do
    local current_time
    current_time=$(date +%s)
    if [ $((current_time - start_time)) -ge "$timeout_seconds" ]; then
      printf 'timeout\n'
      return 0
    fi
    local key_event
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

select_default_mode() {
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

if [ ! -f "$BASE_DIR/config.toml" ]; then
  ui_print "- Fresh installation detected"
  ui_print "- Installing default config..."
  cat "$MODPATH/config.toml" >"$BASE_DIR/config.toml"
  select_default_mode
else
  ui_print "- Existing config found"
  ui_print "- Skipping setup wizard to preserve settings"
fi

set_perm_recursive "$MODPATH" 0 0 0755 0644
set_perm "$BIN_TARGET" 0 0 0755
ui_print "- Installation complete"
