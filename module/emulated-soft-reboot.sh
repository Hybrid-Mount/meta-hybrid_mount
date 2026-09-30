#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# ksud invokes stage scripts by name, not Rust subcommands.
MODDIR="${0%/*}"
exec "$MODDIR/hybrid-mount" runtime prepare-reboot
