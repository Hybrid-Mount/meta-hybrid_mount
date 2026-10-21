#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu

if [ "$#" = 1 ] && [ "$1" = --help ]; then
    echo "Usage: sh $0 /absolute/hybrid-mount /quiet/native/parent"
    exit 0
fi
[ "$#" = 2 ] || exit 2
hm=$1
parent=$2
case "$hm:$parent" in
    /*:/*) ;;
    *) exit 2 ;;
esac
case "$parent" in
    *[!a-zA-Z0-9_./+-]*) exit 2 ;;
esac
[ "$(id -u)" = 0 ] && [ -x "$hm" ] && [ -d "$parent" ] || exit 77
"$hm" vfs version >/dev/null || exit 77
work=$(mktemp -d /data/local/tmp/hm-parent-delete.XXXXXX)
target="${parent%/}/${work##*/}"
if [ -e "$target" ] || [ -L "$target" ]; then
    rmdir "$work"
    exit 77
fi

cleanup() {
    exec 3<&-
    failed=0
    "$hm" vfs rule del "$target/child" >/dev/null 2>&1 || failed=1
    "$hm" vfs rule del "$target" >/dev/null 2>&1 || failed=1
    if [ "$failed" != 0 ]; then
        echo "Cleanup failed; backing files retained at $work" >&2
        return 1
    fi
    rm -f "$work/backing/native" "$work/child"
    rmdir "$work/backing" "$work"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
mkdir "$work/backing"
echo native > "$work/backing/native"
echo surviving-child > "$work/child"

"$hm" vfs rule add "$target" "$work/backing"
"$hm" vfs rule add "$target/child" "$work/child"
exec 3<"$target"
"$hm" vfs rule del "$target"
[ "$(cat "$target/child")" = surviving-child ]
[ ! -e "$target/native" ]
"$hm" vfs rule del "$target/child"
[ ! -e "$target" ]
exec 3<&-
echo 'PASS: deleting a parent preserves children and retires an open old inode safely'
