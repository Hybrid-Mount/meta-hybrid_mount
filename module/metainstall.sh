#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

# metainstall.sh - partition symlink-only handling.
# Hard rule: the only permitted operation is `ln -sf "./system/$partition" "$MODPATH/$partition"`.
# No cp -a && rm -rf, no mv system/<partition>, and no normalisation logic.

if [ "$KSU" = "true" ]; then
  export KSU_HAS_METAMODULE="true"
  export KSU_METAMODULE="hybrid-mount"
fi

if [ "$APATCH" = "true" ]; then
  export APATCH_HAS_METAMODULE="true"
  export APATCH_METAMODULE="hybrid-mount"
fi

export HYBRID_MOUNT="true"

MANAGED_PARTITIONS="odm product system_ext vendor mi_ext my_bigball my_carrier my_company my_engineering my_heytap my_manifest my_preload my_product my_region my_reserve my_stock oem optics prism"

ui_print "- Hybrid Mount metainstall"

# KernelSU's built-in installer resolves these functions dynamically while
# install_module runs. Keep the canonical system/<partition> hierarchy intact
# and translate the official REPLACE variable into OverlayFS opaque metadata.
handle_partition() {
  :
}

mark_replace() {
  replace_target="$1"
  mkdir -p "$replace_target" || return 1
  setfattr -n trusted.overlay.opaque -v y "$replace_target"
}

install_module

for partition in $MANAGED_PARTITIONS; do
  # A module may already provide the promoted partition at the top level and
  # keep system/<partition> as its compatibility alias.  Passing an existing
  # directory to `ln` creates the link *inside* that directory (for example
  # product/product), which later forces a mount over the whole partition.
  if [ -e "$MODPATH/$partition" ] || [ -L "$MODPATH/$partition" ]; then
    continue
  fi

  if [ ! -d "$MODPATH/system/$partition" ]; then
    continue
  fi

  if [ -d "/$partition" ] && [ -L "/system/$partition" ]; then
    ln -sf "./system/$partition" "$MODPATH/$partition"
    ui_print "- linked /$partition"
  fi
done

# An empty system directory has nothing to mount, so remove it to skip the system mount.
if [ -d "$MODPATH/system" ] && [ -z "$(ls -A "$MODPATH/system" 2>/dev/null)" ]; then
  rmdir "$MODPATH/system" 2>/dev/null
  ui_print "- removed empty /system directory (skip system mount)"
fi

ui_print "- installation partition layout ready"
