#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/kernel/fs" "$WORK/kernel/include/linux"
printf 'obj-y += ext4/\n' > "$WORK/kernel/fs/Makefile"
printf 'menu "File systems"\nendmenu\n' > "$WORK/kernel/fs/Kconfig"
(cd "$WORK/kernel" && sh "$ROOT/module/vfs/setup.sh" >/dev/null)
for mode in present multiline absent; do
    case "$mode" in
        present) printf 'struct xattr_handler { int (*__get)(void); };\n' > "$WORK/kernel/include/linux/xattr.h" ;;
        multiline) printf 'struct xattr_handler {\n int (*\n __get\n )(void);\n};\n' > "$WORK/kernel/include/linux/xattr.h" ;;
        absent) printf 'struct xattr_handler { int (*get)(void); };\n' > "$WORK/kernel/include/linux/xattr.h" ;;
    esac
    for makefile in "$ROOT/module/vfs/src/Makefile" "$WORK/kernel/fs/hybridmount/Makefile"; do
        cp "$makefile" "$WORK/probe.mk"
        printf '\n.PHONY: feature\nfeature:\n\t@echo $(ccflags-y)\n' >> "$WORK/probe.mk"
        flags=$(make -s -f "$WORK/probe.mk" srctree="$WORK/kernel" feature)
        case "$mode:$flags" in
            absent:*HM_HAVE_XATTR__GET*) echo 'FAIL: missing member enabled' >&2; exit 1 ;;
            absent:*) ;;
            *:*HM_HAVE_XATTR__GET*) ;;
            *) echo "FAIL: $mode member not detected in $makefile" >&2; exit 1 ;;
        esac
    done
done
echo 'PASS: xattr member detection in external and generated in-tree Kbuild'
