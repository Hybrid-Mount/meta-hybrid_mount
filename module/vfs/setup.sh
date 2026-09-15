#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Built-in integration of the Hybrid Mount VFS kernel subsystem (K2).
#
# Run this from the root of a kernel tree:
#
#   sh /path/to/metamodule/module/vfs/setup.sh
#   sh /path/to/metamodule/module/vfs/setup.sh --cleanup
#
# It copies src/ into fs/hybridmount/ and wires up that directory's Kconfig and
# Makefile. The script refuses to touch a tree that already integrates NoMount:
# both implementations hijack inode operations, and the kernel will not stop them
# from coexisting because they register different key types.

set -eu

KERNEL_ROOT=$(pwd)
SELF_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
SRC_DIR="$SELF_DIR/src"
DIR_NAME="hybridmount"
# The make and Kconfig syntax below must stay literal: it is compared with grep -F
# and appended verbatim.
# shellcheck disable=SC2016
MAKEFILE_LINE='obj-$(CONFIG_HYBRIDMOUNT) += hybridmount/'
KCONFIG_LINE='source "fs/hybridmount/Kconfig"'

usage() {
    cat <<'EOF'
Usage: setup.sh [--cleanup | --help]

  (no args)  Integrate the Hybrid Mount VFS subsystem into the kernel tree at $PWD.
  --cleanup  Revert the files and the Kconfig/Makefile edits this script made.
  --help     Show this message.
EOF
}

resolve_fs_dir() {
    if [ -d "$KERNEL_ROOT/fs" ]; then
        FS_DIR="$KERNEL_ROOT/fs"
    elif [ -d "$KERNEL_ROOT/common/fs" ]; then
        FS_DIR="$KERNEL_ROOT/common/fs"
    else
        echo "[ERROR] no fs/ directory here; run this from a kernel tree root" >&2
        exit 1
    fi
    FS_MAKEFILE="$FS_DIR/Makefile"
    FS_KCONFIG="$FS_DIR/Kconfig"
}

has_nomount() {
    [ -d "$FS_DIR/nomount" ] && return 0
    if [ -f "$KERNEL_ROOT/.config" ] && grep -q '^CONFIG_NOMOUNT=' "$KERNEL_ROOT/.config"; then
        return 0
    fi
    if [ -f "$FS_MAKEFILE" ] && grep -q 'nomount' "$FS_MAKEFILE"; then
        return 0
    fi
    if [ -f "$FS_KCONFIG" ] && grep -q 'fs/nomount/Kconfig' "$FS_KCONFIG"; then
        return 0
    fi
    return 1
}

remove_line() {
    target=$1
    line=$2
    if [ -f "$target" ] && grep -qF "$line" "$target"; then
        grep -vF "$line" "$target" > "$target.tmp"
        mv "$target.tmp" "$target"
        echo "[-] reverted $target"
    fi
}

do_cleanup() {
    if [ -d "$FS_DIR/$DIR_NAME" ]; then
        rm -rf "${FS_DIR:?}/$DIR_NAME"
        echo "[-] removed fs/$DIR_NAME"
    fi
    remove_line "$FS_MAKEFILE" "$MAKEFILE_LINE"
    remove_line "$FS_KCONFIG" "$KCONFIG_LINE"
    echo "[+] cleanup complete"
}

do_setup() {
    if has_nomount; then
        echo "[ERROR] this kernel tree already integrates NoMount." >&2
        echo "        Both implementations hijack inode operations and must not" >&2
        echo "        coexist; remove NoMount before integrating this subsystem." >&2
        exit 1
    fi

    if [ -d "$FS_DIR/$DIR_NAME" ]; then
        echo "[ERROR] fs/$DIR_NAME already exists; run --cleanup first" >&2
        exit 1
    fi

    mkdir -p "$FS_DIR/$DIR_NAME"
    cp -f "$SRC_DIR"/*.c "$SRC_DIR"/*.h "$SRC_DIR/Kconfig" "$SRC_DIR/LICENSE" "$FS_DIR/$DIR_NAME/"
    # The out-of-tree Makefile targets a DDK build; in-tree Kbuild needs the
    # CONFIG_HYBRIDMOUNT form instead.
    # shellcheck disable=SC2016
    printf 'obj-$(CONFIG_HYBRIDMOUNT) += hybridmount.o\nccflags-y += -Wno-declaration-after-statement\n' \
        > "$FS_DIR/$DIR_NAME/Makefile"
    echo "[+] copied sources into fs/$DIR_NAME"

    if ! grep -qF "$MAKEFILE_LINE" "$FS_MAKEFILE"; then
        printf '\n%s\n' "$MAKEFILE_LINE" >> "$FS_MAKEFILE"
        echo "[+] updated fs/Makefile"
    fi

    if ! grep -qF "$KCONFIG_LINE" "$FS_KCONFIG"; then
        awk '
            /^endmenu/ { last = NR }
            { lines[NR] = $0 }
            END {
                for (i = 1; i <= NR; i++) {
                    if (i == last) print "source \"fs/hybridmount/Kconfig\""
                    print lines[i]
                }
            }
        ' "$FS_KCONFIG" > "$FS_KCONFIG.tmp"
        mv "$FS_KCONFIG.tmp" "$FS_KCONFIG"
        echo "[+] updated fs/Kconfig"
    fi

    echo "[+] done; enable CONFIG_HYBRIDMOUNT=y (built-in) or =m (module) and build"
}

case "${1-}" in
    --cleanup) resolve_fs_dir; do_cleanup ;;
    --help|-h) usage ;;
    "") resolve_fs_dir; do_setup ;;
    *) usage; exit 1 ;;
esac
