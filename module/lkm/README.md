# Bundled ext4 sysfs LKM

This subtree contains the `nuke_ext4` Linux kernel module source and prebuilt
Android/GKI variants used on APatch and other non-KernelSU environments.
KernelSU installation removes this entire subtree and uses only its supported
`NukeExt4Sysfs` ioctl. Non-KSU installations retain it and attempt the exact
matching prebuilt automatically for ext4 staging.

## License and provenance

- `src/nuke.c` and the files in `binaries/` are derived from
  [backslashxx/mountify](https://github.com/backslashxx/mountify) at snapshot
  [`df5d309802623432d49447ed0e5c5d28841ed60e`](https://github.com/backslashxx/mountify/commit/df5d309802623432d49447ed0e5c5d28841ed60e).
- The last upstream change affecting these LKM assets in that snapshot is
  [`9310c7cf13c0a332aa79cc888d9b67c5b36b95e5`](https://github.com/backslashxx/mountify/commit/9310c7cf13c0a332aa79cc888d9b67c5b36b95e5).
- This LKM subtree is distributed under **GPL-2.0-only**. The complete license
  text is in [`src/LICENSE`](src/LICENSE); the corresponding source directory
  also retains the upstream `Kconfig`, `Makefile`, and README. It is a
  separately identified component from Hybrid Mount's GPL-3.0-only userspace
  core and Apache-2.0 WebUI.
- SHA-256 digests for every prebuilt module are recorded in
  [`binaries/list.txt`](binaries/list.txt).

## Compatibility and risk boundary

The prebuilt selected at runtime must match both the kernel line and its
Android/GKI ABI. A matching version number alone does not guarantee ABI
compatibility. All bundled `.ko` files are **aarch64-only**, and the loader
refuses other architectures. The fallback also needs a readable
`ext4_unregister_sysfs` symbol in `/proc/kallsyms`, `CONFIG_KALLSYMS=y`,
module-loading support, and an `insmod` accepted by the device's kernel and
SELinux policy. A mismatched kernel module can reject loading or crash the
kernel. Automatic selection is therefore exact and refuses unknown
combinations.

`HYBRID_MOUNT_LKM_PATH=/absolute/path/to/nuke.ko` can override automatic
selection for controlled device testing. The loader restores
`/proc/sys/kernel/kptr_restrict` after a temporary symbol-address read attempt,
and treats concealment as best-effort so it never rolls back an otherwise
successful staging mount.

Immediately before `insmod`, the loader atomically creates
`/data/adb/hybrid-mount/lkm_boot_guard` and removes it after the command
returns. If the kernel crashes during loading, the marker survives and the
next boot refuses another automatic LKM attempt while Hybrid Mount continues
without sysfs concealment. After checking ABI compatibility, remove that marker
manually to permit a retry.

Automatic selection is deliberately exact:

| Kernel line | Android/GKI label | Prebuilt |
| --- | --- | --- |
| 4.14 | any (legacy build) | `nuke-android-4.14.ko` |
| 5.10 | 12 | `nuke-android12-5.10.ko` |
| 5.10 | 13 | `nuke-android13-5.10.ko` |
| 5.15 | 13 | `nuke-android13-5.15.ko` |
| 5.15 | 14 | `nuke-android14-5.15.ko` |
| 6.1 | 14 | `nuke-android14-6.1.ko` |
| 6.6 | 15 | `nuke-android15-6.6.ko` |
| 6.12 | 16 | `nuke-android16-6.12.ko` |

The Android label is read from the kernel release's `androidNN` token first,
then from `ro.build.version.release`. Unknown combinations are refused rather
than mapped to a merely similar binary.
