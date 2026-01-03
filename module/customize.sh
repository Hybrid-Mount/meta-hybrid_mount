ui_print "- Extracting module files..."
unzip -o "$ZIPFILE" -d "$MODPATH" >&2
case "$ARCH" in
"arm64")
  ABI="arm64-v8a"
  ;;
"x64")
  ABI="x86_64"
  ;;
"riscv64")
  ABI="riscv64"
  ;;
*)
  abort "! Unsupported architecture: $ARCH"
  ;;
esac
ui_print "- Device Architecture: $ARCH ($ABI)"
BIN_SOURCE="$MODPATH/binaries/$ABI/meta-hybrid"
BIN_TARGET="$MODPATH/meta-hybrid"
if [ ! -f "$BIN_SOURCE" ]; then
  abort "! Binary for $ABI not found in this zip!"
fi
ui_print "- Installing binary for $ABI..."
cp -f "$BIN_SOURCE" "$BIN_TARGET"
set_perm "$BIN_TARGET" 0 0 0755
rm -rf "$MODPATH/binaries"
rm -rf "$MODPATH/system"
BASE_DIR="/data/adb/meta-hybrid"
mkdir -p "$BASE_DIR"
if [ ! -f "$BASE_DIR/config.toml" ]; then
  ui_print "- Installing default config"
  cat "$MODPATH/config.toml" >"$BASE_DIR/config.toml"
fi

IMG_FILE="$BASE_DIR/modules.img"
if [ -f "$IMG_FILE" ]; then
  ui_print "- Existing modules.img found, keeping it."
else
  ui_print "- Creating 2G modules.img (ext4, no journal)..."
  truncate -s 2G "$IMG_FILE"
  if ! mkfs.ext4 -O ^has_journal "$IMG_FILE"; then
    ui_print "! Failed to format modules.img using system mkfs.ext4"
    rm -f "$IMG_FILE"
  fi
fi

if [ -z "$KSU" ]; then
  touch /data/adb/.litemode_enable
  chmod 600 /data/adb/.litemode_enable
  rm -rf "$MODPATH/metainstall.sh"
  rm -rf "$MODPATH/metamount.sh"
  rm -rf "$MODPATH/metauninstall.sh"
fi
if [ -n "$KSU" ]; then
  rm -rf "$MODPATH/service.sh"
  rm -rf "$MODPATH/post-fs-data.sh"
fi
set_perm_recursive "$MODPATH" 0 0 0755 0644
set_perm "$BIN_TARGET" 0 0 0755
set_perm "$MODPATH/tools/mkfs.erofs" 0 0 0755
ui_print "- Installation complete"