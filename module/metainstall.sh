
export KSU_HAS_METAMODULE="true"
export KSU_METAMODULE="meta-hybrid"
BASE_DIR="/data/adb/meta-hybrid"

BUILTIN_PARTITIONS=""system" "vendor" "product" "system_ext" "odm" "oem" "apex""

# we no-op handle_partition
# because ksu moves them e.g. MODDIR/system/product to MODDIR/product
# this way we can support normal hierarchy that ksu changes
handle_partition() {
    echo 0 > /dev/null ; true
}

# we move the partition folders out of system/
# but don't create symlinks to avoid unnecessary mount
hybrid_handle_partition() {
    partition="$1"

    if [ ! -d "$MODPATH/system/$partition" ]; then
        return
    fi

    if [ -d "$MODPATH/system/$partition" ] && [ ! -L "$MODPATH/system/$partition" ]; then
        mv -f "$MODPATH/system/$partition" "$MODPATH/$partition"
        ui_print "- handled /$partition"
    fi
}

cleanup_empty_system_dir() {
    if [ -d "$MODPATH/system" ] && [ -z "$(ls -A "$MODPATH/system" 2>/dev/null)" ]; then
        rmdir "$MODPATH/system" 2>/dev/null
    fi
}


ui_print "- Using Hybrid Mount metainstall"
install_module
for partition in $BUILTIN_PARTITIONS; do
    hybrid_handle_partition "$partition"
done
cleanup_empty_system_dir
ui_print "- Installation complete"
