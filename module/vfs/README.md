# VFS kernel subsystem (K2)

Status: **in progress**. The upstream baseline is imported and the Hybrid Mount
identity rename is applied; the behaviour changes and the built-in integration script
are still pending, and nothing here is built into a release yet.

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

- Identity: the source files are hybridmount.c / hybridmount.h, the key type is
  "hybridmount", the reported protocol version is "hm1", the Kconfig symbol is
  HYBRIDMOUNT and the module object is hybridmount.o.
- The wire payload layout and the internal symbol prefixes are not changed yet.

## Divergence still to apply (Phase 3)

1. An explicit opaque-directory flag implementing .replace: the directory stays
   visible while its real children are hidden and only injected children are shown.
   Upstream represents .replace as a directory whiteout, which cannot take injected
   children.
2. Batch rule application that preserves the first error and reports the failing
   record, instead of letting the last record overwrite the status.
3. UID isolation lookup that avoids a linear scan in the per-lookup hot path.
4. Diagnostics consumed by the hybrid-mount vfs status and doctor commands.
5. A decision on the wire magic value (currently the upstream constant).

## Packaging and installation (to be wired up in Phase 3/4)

- The release build stages all of module/ recursively, so this src/ tree would be
  shipped inside the release ZIP as-is. The installer should prune it and retain only
  binaries/, mirroring what customize.sh already does for the ext4 LKM subtree.
- shellcheck currently lints only module/*.sh, so a new module/vfs/setup.sh needs an
  explicit CI entry.
- The built-in integration script must refuse to patch a kernel tree that already
  integrates NoMount, and the module must not register the upstream key type.
