# VFS kernel subsystem (K2)

Hybrid Mount's own VFS path-redirection kernel module. It is forked from NoMount
and driven exclusively by the Hybrid Mount metamodule over the keyring; it does not
interoperate with NoMount's metamodule or its nm CLI.

The sources compile against every DDK target (see .github/workflows/kernel-module.yml)
and the metamodule binds to whichever implementation the kernel provides.

## Layout

- src/ — the forked kernel sources
- src/PROVENANCE — fork commit, baseline digests and sync instructions
- src/UPSTREAM_README.md — upstream kernel integration README, retained verbatim
- src/LICENSE — upstream license text
- binaries/ — prebuilt hybridmount-<android>-<kernel>.ko plus list.txt (SHA-256),
  produced by the DDK workflow and consumed by the runtime loader
- setup.sh — built-in integration into a kernel tree

## License and provenance

- The sources in src/ are derived from
  [maxsteeel/nomount](https://github.com/maxsteeel/nomount) at snapshot
  [016375cd4a9e7da07b0519dd7bc492101de2a834](https://github.com/maxsteeel/nomount/commit/016375cd4a9e7da07b0519dd7bc492101de2a834).
- Upstream ships a GPL-3.0 license text in src/LICENSE, but the kernel module declares
  MODULE_LICENSE("GPL") — GPL-2.0-or-later in kernel convention. The two statements are
  inconsistent. This is recorded in [THIRD_PARTY.md](../../THIRD_PARTY.md) and must be
  clarified with the upstream author before any compiled artifact is distributed.
- MODULE_AUTHOR("maxsteeel") and the upstream licence and attribution notices are kept.
- K2 is a separately identified component from Hybrid Mount's GPL-3.0-only userspace
  core and Apache-2.0 WebUI.
- Baseline SHA-256 digests are recorded in src/PROVENANCE.

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
- Builds for pre-5.18 kernels need -std=gnu11, which the Makefile sets.
- Not changed: the wire payload layout (field order, sizes and command numbering).

## Divergence still to apply

1. UID isolation lookup that avoids a linear scan in the per-lookup hot path.
2. Diagnostics consumed by the hybrid-mount vfs status and doctor commands.

## Building

**Prebuilt (default).** .github/workflows/kernel-module.yml builds one module per DDK
target and the packaging job assembles them into binaries/ with a list.txt digest file.
Run the workflow manually with commit_binaries=true to refresh the copies kept in the
repository. The runtime loader matches the device's kernel release and Android version
against that matrix, loads the exact match and treats the keyring response as
authoritative.

**Locally, with DDK:**

~~~
ddk build --target android14-6.1 -- -C module/vfs/src
~~~

**Locally, against a prepared kernel tree:**

~~~
make -C module/vfs/src KDIR=/path/to/kernel
~~~

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

- xtask keeps module/vfs/src out of the release ZIP; only binaries/ ships.
- customize.sh prunes the dev-only sources from the installed module on every platform
  and keeps the prebuilt modules.
- lints.yml verifies binaries/list.txt whenever it is committed.
