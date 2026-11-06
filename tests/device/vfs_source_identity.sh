#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Run only on a disposable kernel with this exact Hybrid Mount build loaded.
set -eu
exec python3 "$(dirname "$0")/vfs_probe.py" source
