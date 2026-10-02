#!/system/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Manual regression for the lifetime of hooked real parent-directory inodes.
# Run on a rooted test device with the freshly built Hybrid Mount provider:
#   sh vfs_parent_lifetime.sh /path/to/hybrid-mount /quiet/readonly/parent [rounds]
#
# The parent must already exist on a read-only disk filesystem (EROFS preferred),
# and should have no unrelated open directory/file references or mmap users.
# In particular, choose a quiet directory with zero mmap users of its children;
# unrelated references can prevent reclamation and let the old implementation
# pass. A tmpfs fixture cannot exercise normal disk-inode cache reclamation.
# This test closes its own file references before each cache drop, then checks
# actual file contents through fresh opens. A passing run alone does not prove
# that the old kernel reclaimed this parent; compare against the old provider
# and, where available, use inode-eviction tracing and KASAN/kmemleak.
#
# WARNING: writing 3 to /proc/sys/vm/drop_caches drops caches globally, which can
# affect device performance. Use a test device. No existing native file is
# modified: only unique, initially absent child paths are injected. Cleanup
# deletes precisely these rules; it never clears the provider or unloads it.
# Exit 77 means an environment prerequisite is missing; usage errors exit 2.

set -eu

usage() {
    printf '%s\n' "Usage: sh $0 /absolute/hybrid-mount /quiet/readonly/parent [rounds: 1-100]"
}

skip() {
    printf 'SKIP: %s\n' "$*" >&2
    exit 77
}

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

if [ "$#" -eq 1 ] && [ "$1" = '--help' ]; then
    usage
    exit 0
fi
if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
    usage >&2
    exit 2
fi

hm=$1
parent_arg=$2
rounds=${3:-5}
case "$hm" in
    /*) ;;
    *) usage >&2; exit 2 ;;
esac
case "$parent_arg" in
    /*) ;;
    *) usage >&2; exit 2 ;;
esac
case "$parent_arg" in
    *[!a-zA-Z0-9_./+-]*)
        printf '%s\n' 'Parent paths must use ordinary, unescaped pathname characters.' >&2
        exit 2
        ;;
esac
case "$rounds" in
    ''|*[!0-9]*) usage >&2; exit 2 ;;
esac
if [ "${#rounds}" -gt 3 ] || [ "$rounds" -lt 1 ] || [ "$rounds" -gt 100 ]; then
    usage >&2
    exit 2
fi

for tool in id awk mktemp cat cmp sync grep rm; do
    command -v "$tool" >/dev/null 2>&1 || skip "missing command: $tool"
done
[ "$(id -u)" = 0 ] || skip 'run from an explicit root shell'
[ -x "$hm" ] || skip "Hybrid Mount executable is unavailable: $hm"
[ -d "$parent_arg" ] || skip "parent directory does not exist: $parent_arg"
parent=$(cd "$parent_arg" && pwd -P) || skip 'cannot resolve the parent directory'
case "$parent" in
    /|*[!a-zA-Z0-9_./+-]*) skip 'choose a quiet directory below / with an unescaped physical path' ;;
esac
cd /
[ -r /proc/self/mountinfo ] || skip '/proc/self/mountinfo is unavailable'
[ -w /proc/sys/vm/drop_caches ] || skip 'global drop_caches is unavailable or unwritable'
[ -d /data/local/tmp ] || skip '/data/local/tmp is unavailable'

# Select the deepest mount containing the supplied physical parent. Reject
# overlay/tmpfs and writable mounts even when their native files are read-only.
mount_details=$(awk -v target="$parent" '
    {
        point = $5
        if (point != "/" && target != point && index(target, point "/") != 1)
            next
        if (length(point) < best)
            next
        for (i = 7; i <= NF; i++) {
            if ($i == "-") {
                best = length(point)
                type = $(i + 1)
                options = $6
                break
            }
        }
    }
    END { if (best) print type " " options }
' /proc/self/mountinfo) || skip 'cannot inspect the parent mount'
[ -n "$mount_details" ] || skip 'cannot identify the parent mount'
fs_type=${mount_details%% *}
mount_options=${mount_details#* }
case "$fs_type" in
    erofs|squashfs|ext2|ext3|ext4|f2fs|xfs|btrfs) ;;
    *) skip "parent must be on a disk filesystem, found $fs_type" ;;
esac
case ",$mount_options," in
    *,ro,*) ;;
    *) skip 'the parent mount must be read-only' ;;
esac

"$hm" vfs version >/dev/null || skip 'a supported, already loaded VFS provider is required'
isolated_uids=$("$hm" vfs uid list --json) || skip 'cannot read the isolated UID table'
if printf '%s\n' "$isolated_uids" | grep -Eq '(^|[^0-9])0([^0-9]|$)'; then
    skip 'UID 0 is isolated; leave the existing isolation table unchanged'
fi

work=$(mktemp -d /data/local/tmp/hm-vfs-parent.XXXXXX) || skip 'cannot create replacement sources'
one_attempted=0
two_attempted=0
deep_attempted=0
one=''
two=''
deep=''

cleanup_rule() {
    target=$1
    if "$hm" vfs rule del "$target" --uid 0 >/dev/null 2>"$work/cleanup-error.txt"; then
        return
    fi
    # A failed ADD_RULE/readback may have installed nothing. Only regard an
    # unsuccessful delete as clean when a fresh listing confirms absence.
    if "$hm" vfs rule list >"$work/cleanup-rules.txt" 2>>"$work/cleanup-error.txt" &&
        ! grep -F "\"$target\" " "$work/cleanup-rules.txt" >/dev/null; then
        return
    fi
    printf 'FAIL: cleanup could not confirm deletion of %s\n' "$target" >&2
    cat "$work/cleanup-error.txt" >&2
    cleanup_failed=1
}

cleanup() {
    status=$?
    trap - 0 1 2 15
    set +e
    cleanup_failed=0
    [ "$deep_attempted" -eq 0 ] || cleanup_rule "$deep"
    [ "$two_attempted" -eq 0 ] || cleanup_rule "$two"
    [ "$one_attempted" -eq 0 ] || cleanup_rule "$one"
    if [ "$cleanup_failed" -eq 0 ]; then
        # Verify the exact, generated absolute cleanup target before recursion.
        work_name=${work#/data/local/tmp/}
        case "$work:$work_name" in
            /data/local/tmp/hm-vfs-parent.*:hm-vfs-parent.*)
                case "$work_name" in
                    */*) printf 'FAIL: refusing unexpected cleanup path %s\n' "$work" >&2; status=1 ;;
                    *) rm -rf -- "$work" || status=1 ;;
                esac
                ;;
            *) printf 'FAIL: refusing unexpected cleanup path %s\n' "$work" >&2; status=1 ;;
        esac
    else
        printf 'Replacement sources and diagnostics retained at %s\n' "$work" >&2
        [ "$status" -ne 0 ] || status=1
    fi
    exit "$status"
}
trap cleanup 0
trap 'exit 129' 1
trap 'exit 130' 2
trap 'exit 143' 15

stem=.${work##*/}
one=$parent/$stem-one
two=$parent/$stem-two
tree=$parent/$stem-tree
deep=$tree/missing/subtree/item
for path in "$one" "$two" "$tree"; do
    if [ -e "$path" ] || [ -L "$path" ]; then
        skip "test path already exists: $path"
    fi
done

"$hm" vfs rule list >"$work/rules-before.txt" || skip 'cannot inspect existing rules'
if grep -F "\"$parent/" "$work/rules-before.txt" >/dev/null; then
    skip 'the parent already contains VFS targets or pinned sources; choose a quiet parent'
fi
ancestor=$parent
while :; do
    if grep -F "\"$ancestor\"" "$work/rules-before.txt" >/dev/null; then
        skip "an existing VFS rule references the parent or an ancestor: $ancestor"
    fi
    [ "$ancestor" != / ] || break
    ancestor=${ancestor%/*}
    [ -n "$ancestor" ] || ancestor=/
done

printf 'first replacement: %s\n' "$stem" >"$work/source-one"
printf 'second replacement: %s\n' "$stem" >"$work/source-two"
printf 'shadow replacement: %s\n' "$stem" >"$work/source-shadow"
printf 'deep replacement: %s\n' "$stem" >"$work/source-deep"

assert_content() {
    target=$1
    source=$2
    # This cat process exits, closing its target file before the next cache drop.
    cat "$target" >"$work/actual" || fail "fresh open failed: $target"
    cmp -s "$work/actual" "$source" || fail "replacement contents lost: $target"
}

assert_absent() {
    if [ -e "$1" ] || [ -L "$1" ]; then
        fail "deleted injection or generated topology is still visible: $1"
    fi
}

drop_caches() {
    sync || fail 'sync failed'
    printf '3\n' >/proc/sys/vm/drop_caches || skip 'kernel refused the global cache drop'
}

# Keep the content-pair loop in a subshell so stress retains its positional
# parameters across rounds; each child exits before the next drop_caches.
stress() {
    label=$1
    shift
    round=1
    while [ "$round" -le "$rounds" ]; do
        drop_caches
        (
            while [ "$#" -gt 0 ]; do
                assert_content "$1" "$2"
                shift 2
            done
        ) || fail "$label"
        round=$((round + 1))
    done
    printf 'PASS: %s (%s cache-drop rounds)\n' "$label" "$rounds"
}

printf 'Testing %s on %s; dropping caches globally.\n' "$parent" "$fs_type"
one_attempted=1
"$hm" vfs rule add "$one" "$work/source-one" --uid 0 >/dev/null || fail 'first add failed'
two_attempted=1
"$hm" vfs rule add "$two" "$work/source-two" --uid 0 >/dev/null || fail 'second add failed'
stress 'two active rules sharing a real parent' "$one" "$work/source-one" "$two" "$work/source-two"

"$hm" vfs rule del "$one" --uid 0 >/dev/null || fail 'first delete failed'
one_attempted=0
assert_absent "$one"
stress 'one deletion preserves the other rule' "$two" "$work/source-two"
"$hm" vfs rule add "$two" "$work/source-shadow" --uid 0 >/dev/null || fail 'shadow add failed'
stress 'shadow re-add refreshes the source' "$two" "$work/source-shadow"

deep_attempted=1
"$hm" vfs rule add "$deep" "$work/source-deep" --uid 0 >/dev/null || fail 'missing subtree add failed'
stress 'missing virtual subtree shares the real parent' "$two" "$work/source-shadow" "$deep" "$work/source-deep"
"$hm" vfs rule del "$two" --uid 0 >/dev/null || fail 'second delete failed'
two_attempted=0
assert_absent "$two"
stress 'virtual subtree keeps the real parent alive' "$deep" "$work/source-deep"
"$hm" vfs rule del "$deep" --uid 0 >/dev/null || fail 'last subtree delete failed'
deep_attempted=0
drop_caches
assert_absent "$tree"
assert_absent "$one"
assert_absent "$two"
printf '%s\n' 'PASS: last deletion prunes the missing virtual subtree'

one_attempted=1
"$hm" vfs rule add "$one" "$work/source-one" --uid 0 >/dev/null || fail 're-add after teardown failed'
stress 're-add after complete teardown' "$one" "$work/source-one"
"$hm" vfs rule del "$one" --uid 0 >/dev/null || fail 'final delete failed'
one_attempted=0
drop_caches
assert_absent "$one"
printf '%s\n' 'PASS: real-parent lifetime and exact rule cleanup'
