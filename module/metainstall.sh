#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

# metainstall.sh — 分区 symlink-only 处理。
# 铁律:只允许 `ln -sf "./system/$partition" "$MODPATH/$partition"`;
# 禁止 cp -a && rm -rf、禁止 mv system/<partition>、禁止归一化逻辑。

export KSU_HAS_METAMODULE="true"
export KSU_METAMODULE="hybrid-mount"

MANAGED_PARTITIONS="odm product system_ext vendor apex mi_ext my_bigball my_carrier my_company my_engineering my_heytap my_manifest my_preload my_product my_region my_reserve my_stock oem optics prism"

ui_print "- Hybrid Mount metainstall"

install_module

for partition in $MANAGED_PARTITIONS; do
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
