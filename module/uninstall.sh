
BASE_DIR="/data/adb/meta-hybrid"
MNT_DIR="$BASE_DIR/mnt"
if mountpoint -q "$MNT_DIR"; then
    umount "$MNT_DIR" 2>/dev/null || umount -l "$MNT_DIR"
fi
rm -rf "$BASE_DIR"
exit 0