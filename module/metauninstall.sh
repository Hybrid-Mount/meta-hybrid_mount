
MNT_DIR="/data/adb/meta-hybrid/mnt"
if [ -z "$MODULE_ID" ]; then
    exit 0
fi
if ! mountpoint -q "$MNT_DIR" 2>/dev/null; then
    exit 0
fi
MOD_IMG_DIR="$MNT_DIR/$MODULE_ID"
if [ -d "$MOD_IMG_DIR" ]; then
    rm -rf "$MOD_IMG_DIR"
fi
exit 0