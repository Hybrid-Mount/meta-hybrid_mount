#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Requires root and Python 3; does not load/unload a provider or clear tables.
set -eu
exec python3 "$(dirname "$0")/vfs_probe.py" uid
