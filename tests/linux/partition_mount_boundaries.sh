#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
if [ "$(id -u)" != 0 ]; then
    echo 'SKIP: run as root in a disposable Linux environment for private mount namespaces' >&2
    exit 77
fi
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$ROOT"
cargo test -p hybrid-mount partitions::tests
cargo test -p hybrid-mount rejected_partition_cannot_leak
cargo test -p hybrid-mount staged_overlay_preserves
cargo test -p hybrid-mount child_failure_never_publishes
cargo test -p hybrid-mount scanned_directory_remove
cargo test -p hybrid-mount directory_remove_stages
cargo test -p hybrid-mount native_directory_mount_mirror
