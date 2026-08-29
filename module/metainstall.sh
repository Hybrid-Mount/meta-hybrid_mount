#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

# metainstall.sh — 分区 symlink-only 处理。
# 铁律:只允许 `ln -sf "./system/$partition" "$MODPATH/$partition"`;
# 禁止 cp -a && rm -rf、禁止 mv system/<partition>、禁止归一化逻辑。

if [ "$KSU" = "true" ]; then
  export KSU_HAS_METAMODULE="true"
  export KSU_METAMODULE="hybrid-mount"
fi

if [ "$APATCH" = "true" ]; then
  export APATCH_HAS_METAMODULE="true"
  export APATCH_METAMODULE="hybrid-mount"
fi

export HYBRID_MOUNT="true"

MANAGED_PARTITIONS="odm product system_ext vendor apex mi_ext my_bigball my_carrier my_company my_engineering my_heytap my_manifest my_preload my_product my_region my_reserve my_stock oem optics prism"

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

# 空的 system 目录没有任何挂载内容,移除空目录以跳过 system 挂载。
if [ -d "$MODPATH/system" ] && [ -z "$(ls -A "$MODPATH/system" 2>/dev/null)" ]; then
  rmdir "$MODPATH/system" 2>/dev/null
  ui_print "- removed empty /system directory (skip system mount)"
fi

ui_print "- installation partition layout ready"
