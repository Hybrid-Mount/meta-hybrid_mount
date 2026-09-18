# VFS kernel subsystem (`hybridmount`)

Hybrid Mount's own VFS path-redirection kernel module. It is forked from NoMount
and driven exclusively by the Hybrid Mount metamodule over the keyring; it does not
interoperate with NoMount's metamodule or its nm CLI.

The release package ships both halves: the sources (for built-in kernel integration)
and one prebuilt `hybridmount-android<NN>-<kernel>.ko` per supported Android/GKI
target, so a device whose kernel does not already carry the module can still use the VFS
backend.

## Layout

- `src/` — the forked kernel sources
- `src/PROVENANCE` — fork commit, baseline digests and sync instructions
- `src/UPSTREAM_README.md` — upstream kernel integration README, retained verbatim
- `src/LICENSE` — GPL-2.0 license text
- `binaries/` — the prebuilt modules and `list.txt` with their SHA-256 digests
- `setup.sh` — built-in integration into a kernel tree

## License and provenance

- The module is distributed under **GPL-2.0-only**. The complete license text is
  [`src/LICENSE`](src/LICENSE), and every source file carries an
  `SPDX-License-Identifier: GPL-2.0-only` header.
- The sources are derived from
  [maxsteeel/nomount](https://github.com/maxsteeel/nomount) at snapshot
  [016375cd4a9e7da07b0519dd7bc492101de2a834](https://github.com/maxsteeel/nomount/commit/016375cd4a9e7da07b0519dd7bc492101de2a834).
  Upstream copyright and `MODULE_AUTHOR("maxsteeel")` are retained.
- The module declares `MODULE_LICENSE("GPL v2")`. The kernel's own docs
  (`include/linux/module.h`) list `"GPL v2"` as a free-software ident equivalent to
  `"GPL"`, and explicitly note that neither string distinguishes "v2 only" from "v2 or
  later" — that distinction comes from the SPDX header and the shipped license text, which
  are both GPL-2.0-only here. `MODULE_LICENSE` exists to mark the module as free and to
  permit binding `EXPORT_SYMBOL_GPL` symbols. Upstream shipped a conflicting GPL-3.0 text
  alongside a bare `MODULE_LICENSE("GPL")`; this module carries the GPL-2.0-only grant
  instead.
- The module is a separately identified component from Hybrid Mount's GPL-3.0-only userspace
  core and Apache-2.0 WebUI. See [THIRD_PARTY.md](../../THIRD_PARTY.md).
- Baseline SHA-256 digests are recorded in `src/PROVENANCE`.

## Divergence applied

- Identity: hybridmount.c / hybridmount.h, key type "hybridmount", protocol version
  "hm1", Kconfig symbol HYBRIDMOUNT, module object hybridmount.o.
- Internal symbols: nomount_* -> hybridmount_*, nm_* -> hm_*, NM_* -> HM_*, and the
  kernel log prefix is "hybridmount:".
- Wire magic: HYBRIDMOUNT_MAGIC_SIG is the HM-exclusive value 0x4859425249444D4F
  (ASCII "HYBRIDMO" read big-endian), replacing upstream's 0x4E4F4D4F554E54
  ("NOMOUNT"). A stock nm CLI is now rejected at preparse with -EFAULT rather than
  being kept out by the key type name alone. The constant is also the full_name_hash
  seed and must stay byte-identical to src/vfs/protocol.rs::MAGIC.
- Virtual offset signature: HM_SIG_16 is 'hm' (0x686D), replacing upstream's 'nm'
  (0x6E6D). This is the high half of the packed virtual loff_t, so it is an in-kernel
  ABI in its own right.
- Batch ADD_RULE reports the first failure together with the offset of the failing
  record, instead of letting the last record overwrite the status. DEL_RULE reports the
  first error the same way and no longer leaves status 0 on a truncated batch.
- A rule whose real path fails to resolve is rejected with -ENOENT instead of being
  inserted as a rule that can never match; ADD_RULE and DEL_RULE reject a cursor past
  data_size; directory rules fail with -ENOMEM when the directory node cannot be
  allocated.
- HM_FLAG_OPAQUE marks a directory that replaces its whole subtree: it stays visible,
  hides every real child and shows only the injected ones. The userspace emits it for a
  .replace directory and for every directory below it, matching Magisk semantics.
- The isolated-uid table stays sorted, so the per-lookup isolation check bisects
  instead of scanning every entry.
- Builds for pre-5.18 kernels need -std=gnu11, which the Makefile sets.
- Not changed: the wire payload layout (field order, sizes and command numbering).

## Compatibility

Prebuilt modules are **aarch64-only**. The loader refuses other architectures, and
`customize.sh` strips `vfs/binaries` on installs that cannot use them.

| Android/GKI target | Prebuilt |
| --- | --- |
| 12 / 5.10 | `hybridmount-android12-5.10.ko` |
| 13 / 5.10 | `hybridmount-android13-5.10.ko` |
| 13 / 5.15 | `hybridmount-android13-5.15.ko` |
| 14 / 5.15 | `hybridmount-android14-5.15.ko` |
| 14 / 6.1 | `hybridmount-android14-6.1.ko` |
| 15 / 6.6 | `hybridmount-android15-6.6.ko` |
| 16 / 6.12 | `hybridmount-android16-6.12.ko` |

Selection requires both the kernel line and its Android/GKI label to match; a merely
similar version is refused. A matching version number still does not guarantee ABI
compatibility, so a mismatched module can fail to load or crash the kernel. Before
`insmod`, the loader writes `/data/adb/hybrid-mount/vfs_boot_guard` and removes it once
the load returns; if the kernel crashes, the marker survives and the next boot skips
the VFS backend while the rest of Hybrid Mount keeps working.

## Building

The CI workflow compile-checks every supported DDK target. For local kernel
development with DDK:

~~~
ddk build --target android14-6.1 -- -C module/vfs/src
~~~

**Locally, against a prepared kernel tree:**

~~~
make -C module/vfs/src KDIR=/path/to/kernel
~~~

## Refreshing the prebuilt modules

`.github/workflows/kernel-module.yml` builds every target with DDK, assembles
`module/vfs/binaries/` plus `list.txt`, and uploads the result as an artifact. Run the
workflow manually with `commit_binaries=true` to commit the refreshed modules back to
the branch. `customize.sh` ships them in the release ZIP and `lints.yml` verifies their
digests.

## Built-in integration

From the root of a kernel tree:

~~~
sh /path/to/metamodule/module/vfs/setup.sh
~~~

It copies the sources into fs/hybridmount/, adds the fs/hybridmount entry to
fs/Makefile, sources fs/hybridmount/Kconfig, then leaves you to enable
CONFIG_HYBRIDMOUNT=y. --cleanup reverts all of it. The script refuses to touch a kernel
tree that already integrates NoMount, because both implementations hijack inode
operations and the kernel will not stop them from coexisting.

## Packaging

- xtask ships `module/vfs/src`, `module/vfs/binaries` and `setup.sh` in the release
  ZIP, and strips any Kbuild output a local build left in the working tree.
- `customize.sh` keeps the sources on every platform and drops `vfs/binaries` where
  the prebuilt modules cannot load.
