#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

# uninstall.sh - uninstall cleanup: remove the runtime directory.
# The manager uninstalls the module source directory; /data/adb/modules is untouched here.

rm -rf "/data/adb/hybrid-mount"

exit 0
