#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

# uninstall.sh — 卸载清理:移除运行目录。
# 模块源目录由管理器负责卸载;此处不触碰 /data/adb/modules。

rm -rf "/data/adb/hybrid-mount"

exit 0
