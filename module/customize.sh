ui_print "- Extracting module files..."
unzip -o "$ZIPFILE" -d "$MODPATH" >&2
case "$ARCH" in
"arm64")
  ABI="arm64-v8a"
  ;;
"x64")
  ABI="x86_64"
  ;;
"arm")
  ABI="armeabi-v7a"
  ;;
*)
  abort "! Unsupported architecture: $ARCH"
  ;;
esac
ui_print "- Device Architecture: $ARCH ($ABI)"
BIN_SOURCE="$MODPATH/binaries/$ABI/hybrid-mount"
BIN_TARGET="$MODPATH/hybrid-mount"
if [ ! -f "$BIN_SOURCE" ]; then
  abort "! Binary for $ABI not found in this zip!"
fi
ui_print "- Installing binary for $ABI..."
cp -f "$BIN_SOURCE" "$BIN_TARGET"
set_perm "$BIN_TARGET" 0 0 0755
rm -rf "$MODPATH/binaries"
rm -rf "$MODPATH/system"
BASE_DIR="/data/adb/hybrid-mount"
mkdir -p "$BASE_DIR"

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
  local start_time=$(date +%s)
  local chosen_mode="overlay"
  while true; do
    local current_time=$(date +%s)
    if [ $((current_time - start_time)) -ge $timeout ]; then
      ui_print "- Timeout: Selected OverlayFS"
      break
    fi
    local key_event=$(timeout 0.5 getevent -l 2>/dev/null)
    if echo "$key_event" | grep -q "KEY_VOLUMEUP"; then
      chosen_mode="overlay"
      ui_print "- Key Detected: Selected OverlayFS"
      break
    elif echo "$key_event" | grep -q "KEY_VOLUMEDOWN"; then
      chosen_mode="magic"
      ui_print "- Key Detected: Selected Magic Mount"
      break
    fi
  done
  ui_print "- Configured mode: $chosen_mode"
  sed -i '/default_mode/d' "$BASE_DIR/config.toml"
  echo "default_mode = \"$chosen_mode\"" >> "$BASE_DIR/config.toml"
}

if [ ! -f "$BASE_DIR/config.toml" ]; then
  ui_print "- Fresh installation detected"
  ui_print "- Installing default config..."
  cat "$MODPATH/config.toml" >"$BASE_DIR/config.toml"
  KEY_volume_detect
else
  ui_print "- Existing config found"
  ui_print "- Skipping setup wizard to preserve settings"
fi

set_perm_recursive "$MODPATH" 0 0 0755 0644
set_perm "$BIN_TARGET" 0 0 0755
ui_print "- Installation complete"
