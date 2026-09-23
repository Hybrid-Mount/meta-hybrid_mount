#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Remove only the obsolete pre-runtime-manager semaphore from older installs.
# Current operation locks are file locks released automatically when the process exits.
rmdir /dev/hybrid_mount_single_instance 2>/dev/null || :
