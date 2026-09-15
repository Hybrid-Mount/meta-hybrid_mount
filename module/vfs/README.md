# VFS kernel subsystem (K2)

Status: **in progress**. The upstream baseline is imported and both the identity
rename and the internal symbol rename are applied; the behaviour changes and the
built-in integration script are still pending, and nothing here is built into a
release yet.

This subtree holds the K2 VFS path-redirection kernel module: its source, the
prebuilt Android/GKI variants and the built-in integration script. K2 is meant to be
driven exclusively by the Hybrid Mount metamodule over the keyring and does not
interoperate with NoMount's metamodule or its nm CLI.

## Layout

- src/ — the forked kernel sources
- src/PROVENANCE — fork commit, baseline digests and sync instructions
- src/UPSTREAM_README.md — upstream kernel integration README, retained verbatim
- src/LICENSE — upstream license text
- binaries/ — prebuilt hybridmount-<gki>-<kernel>.ko (Phase 3; not present yet)

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
- Batch ADD_RULE reports the first failure together with the offset of the failing
  record, instead of letting the last record overwrite the status.
- HM_FLAG_OPAQUE marks a directory that replaces its whole subtree: it stays visible,
  hides every real child and shows only the injected ones. The userspace emits it for a
  .replace directory and for every directory below it, matching Magisk semantics.
- Not changed yet: the wire payload layout and the magic value.

## Divergence still to apply (Phase 3)

1. UID isolation lookup that avoids a linear scan in the per-lookup hot path.
2. Diagnostics consumed by the hybrid-mount vfs status and doctor commands.
3. A decision on the wire magic value (currently the upstream constant).

## Building and integrating

Two supported routes, both driven by the same sources in src/:

1. **Loadable module (recommended for testing).** DDK provides one image per
   Android/GKI target with the matching kernel headers and Clang toolchain:

   ~~~
   ddk build --target android14-6.1 -- -C module/vfs/src
   ~~~

   `.github/workflows/kernel-module.yml` builds the same way for every target and
   uploads `hybridmount-<target>.ko`. Without DDK, pass a prepared kernel tree:
   `make -C module/vfs/src KDIR=/path/to/kernel`.

2. **Built-in.** From the root of a kernel tree:

   ~~~
   sh /path/to/metamodule/module/vfs/setup.sh
   ~~~

   It copies the sources into `fs/hybridmount/`, adds the `fs/hybridmount` entry to
   `fs/Makefile`, sources `fs/hybridmount/Kconfig`, then leaves you to enable
   `CONFIG_HYBRIDMOUNT=y`. `--cleanup` reverts all of it. The script refuses to
   touch a kernel tree that already integrates NoMount, because both implementations
   hijack inode operations and the kernel will not stop them from coexisting.

Once a kernel provides K2, the metamodule binds to it automatically; `vfs_strict`
decides whether an unavailable backend fails the boot or degrades.

## Packaging and installation (to be wired up in Phase 3/4)

- The release build stages all of module/ recursively, so this src/ tree would be
  shipped inside the release ZIP as-is. The installer should prune it and retain only
  binaries/, mirroring what customize.sh already does for the ext4 LKM subtree.
- shellcheck currently lints only module/*.sh, so a new module/vfs/setup.sh needs an
  explicit CI entry.
- The built-in integration script must refuse to patch a kernel tree that already
  integrates NoMount, and the module must not register the upstream key type.
