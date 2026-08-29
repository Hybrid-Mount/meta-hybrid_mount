#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

# KernelSU metamodule hooks may invoke metamount.sh more than once during the
# same boot. Keep the atomic /dev lock through early boot, then release it so a
# deliberate post-boot hot-install run remains possible.

LOCK_DIR="/dev/hybrid_mount_single_instance"
rmdir "$LOCK_DIR" 2>/dev/null
