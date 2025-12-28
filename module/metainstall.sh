export KSU_HAS_METAMODULE="true"
export KSU_METAMODULE="meta-hybrid"
BASE_DIR="/data/adb/meta-hybrid"
BUILTIN_PARTITIONS="system vendor product system_ext odm oem apex"

handle_partition() {
    echo 0 > /dev/null ; true
}

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
        ui_print "- Removed empty /system directory (Skip system mount)"
    fi
}

ui_print "- Using Hybrid Mount metainstall"

install_module

for partition in $BUILTIN_PARTITIONS; do
    hybrid_handle_partition "$partition"
done

cleanup_empty_system_dir

ui_print "- Installation complete"