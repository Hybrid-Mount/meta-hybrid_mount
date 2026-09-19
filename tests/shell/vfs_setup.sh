#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Exercise module/vfs/setup.sh against throwaway kernel trees: integration,
# cleanup, and the refusal to touch a kernel tree that already has NoMount.

set -eu

REPO_ROOT=$(cd -- "$(dirname -- "$0")/../.." && pwd)
SETUP="$REPO_ROOT/module/vfs/setup.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

fail() {
    echo "FAIL: $1" >&2
    exit 1
}

make_kernel_tree() {
    root=$1
    mkdir -p "$root/fs"
    printf 'obj-y += ext4/\n' > "$root/fs/Makefile"
    printf 'menu "File systems"\nsource "fs/ext4/Kconfig"\nendmenu\n' > "$root/fs/Kconfig"
}

# 1. Integration copies the sources and wires up Kconfig and Makefile.
tree="$WORK/kernel"
make_kernel_tree "$tree"
(cd "$tree" && sh "$SETUP" > /dev/null)

[ -f "$tree/fs/hybridmount/hybridmount.c" ] || fail "sources were not copied"
[ -f "$tree/fs/hybridmount/LICENSE" ] || fail "license was not copied"
# shellcheck disable=SC2016
grep -qF 'obj-$(CONFIG_HYBRIDMOUNT) += hybridmount/' "$tree/fs/Makefile" ||
    fail "fs/Makefile was not patched"
grep -qF 'source "fs/hybridmount/Kconfig"' "$tree/fs/Kconfig" ||
    fail "fs/Kconfig was not patched"
# shellcheck disable=SC2016
grep -qF 'obj-$(CONFIG_HYBRIDMOUNT) += hybridmount.o' "$tree/fs/hybridmount/Makefile" ||
    fail "in-tree Makefile is not in Kbuild form"
grep -qF -- '-std=gnu11' "$tree/fs/hybridmount/Makefile" ||
    fail "in-tree Makefile must force gnu11 for pre-5.18 kernels"

# 2. A second run must refuse rather than clobber the tree.
if (cd "$tree" && sh "$SETUP" > /dev/null 2>&1); then
    fail "a second integration run should be refused"
fi

# 3. The script must not require bash.
grep -q '^#!/bin/sh' "$SETUP" || fail "setup.sh should be POSIX sh"

# 4. A kernel tree with NoMount must be refused.
nomount="$WORK/nomount-kernel"
make_kernel_tree "$nomount"
mkdir -p "$nomount/fs/nomount"
if (cd "$nomount" && sh "$SETUP" > /dev/null 2>&1); then
    fail "a NoMount kernel tree should be refused"
fi
[ -d "$nomount/fs/hybridmount" ] && fail "sources leaked into a NoMount tree"

# 5. Cleanup restores the tree.
(cd "$tree" && sh "$SETUP" --cleanup > /dev/null)
[ -d "$tree/fs/hybridmount" ] && fail "cleanup left fs/hybridmount behind"
grep -qF 'hybridmount' "$tree/fs/Makefile" && fail "cleanup left fs/Makefile patched"
grep -qF 'hybridmount' "$tree/fs/Kconfig" && fail "cleanup left fs/Kconfig patched"

# 6. Piped installation fetches sources without a local checkout.
mkdir -p "$WORK/bin"
cat > "$WORK/bin/curl" <<'EOF'
#!/bin/sh
set -eu
[ "$1" = "-fLSs" ]
[ "$3" = "-o" ]
case "$2" in
    https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/src/*) ;;
    *) exit 1 ;;
esac
[ "${FAIL_DOWNLOAD:-0}" = 0 ] || exit 22
cp "$TEST_SOURCE_DIR/${2##*/}" "$4"
EOF
chmod +x "$WORK/bin/curl"
export TEST_SOURCE_DIR="$REPO_ROOT/module/vfs/src"
export PATH="$WORK/bin:$PATH"
remote="$WORK/remote-kernel"
make_kernel_tree "$remote"
(cd "$remote" && bash < "$SETUP" > /dev/null)
cmp "$TEST_SOURCE_DIR/hybridmount.c" "$remote/fs/hybridmount/hybridmount.c" ||
    fail "piped setup did not fetch the correct sources"
# Cleanup must work even if source downloads are unavailable.
(cd "$remote" && FAIL_DOWNLOAD=1 bash -s -- --cleanup < "$SETUP" > /dev/null)
[ ! -d "$remote/fs/hybridmount" ] || fail "piped cleanup left sources behind"

# 7. A failed download must leave the kernel tree untouched.
cp "$remote/fs/Makefile" "$WORK/Makefile.before"
cp "$remote/fs/Kconfig" "$WORK/Kconfig.before"
if (cd "$remote" && FAIL_DOWNLOAD=1 bash < "$SETUP" > /dev/null 2>&1); then
    fail "failed download should fail setup"
fi
[ ! -d "$remote/fs/hybridmount" ] || fail "failed download left partial integration"
cmp "$WORK/Makefile.before" "$remote/fs/Makefile" || fail "failed download modified Makefile"
cmp "$WORK/Kconfig.before" "$remote/fs/Kconfig" || fail "failed download modified Kconfig"

echo "PASS: vfs setup.sh local/piped integration, download failure, guard and cleanup"
