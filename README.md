# Hybrid Mount

<img src="icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount is a hybrid mount meta-module for KernelSU and APatch. During boot, it scans other modules and selects OverlayFS, Magic Mount, VFS, or ignore for each entry according to global, module, and path rules. Module source directories are always treated as read-only input.

## Features

- OverlayFS, Magic Mount, and VFS can be mixed per module and per path.
- Path rules take precedence over module defaults, and module defaults take precedence over the global default.
- OverlayFS supports both tmpfs and ext4 storage modes.
- For ext4 staging, KernelSU uses the official ioctl to hide sysfs nodes; APatch and other non-KSU environments use the bundled LKM compatibility fallback by default.
- Magic Mount supports files, directories, symbolic links, `.replace`, and whiteout semantics.
- VFS sends injection rules to Hybrid Mount's own VFS kernel subsystem, the `hybridmount` module, through the keyring. It is an independent implementation and does not interoperate with NoMount's kernel or its nm CLI. Releases ship the sources and a prebuilt aarch64 module per supported Android/GKI target, which the boot pipeline loads when the kernel does not already carry one; if that fails it falls back according to `vfs_strict`. VFS is not a real mount.
- The WebUI provides MD3 (default) and Miuix interfaces.
- arm64, armv7, and x86_64 are supported; the installer automatically selects the matching binary.

## Installation

Download the ZIP from [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) and install it with the KernelSU or APatch manager. On first installation, use the volume keys to select the default backend. Upgrades preserve `/data/adb/hybrid-mount/config.toml`.

## Configuration

Default configuration:

```toml
moduledir = "/data/adb/modules"
overlay_mode = "ext4" # ext4 | tmpfs
disable_umount = false
default_mode = "overlay" # overlay | magic | vfs

[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

Rule paths are relative to the module root. Module-level and path-level rules may also use `ignore`; the global default backend accepts `overlay`, `magic`, or `vfs`. `vfs_strict = true` makes startup fail when VFS is unavailable, and `vfs_isolate_uids` lists the UIDs that should see the native filesystem. The same file path cannot be assigned to more than one backend. Overlay and Magic, the two real mount backends, may share ordinary directories as structural nodes; VFS is a third injection path rather than a real mount. File, type, and `.replace` conflicts cause the startup planning stage to fail immediately. Configuration changes take effect after reboot.

This routing does not change the project's existing `CONFIG_TMPFS_XATTR` capability check. On KernelSU, installation removes the module's entire `lkm/` directory and runtime uses only the official `NukeExt4Sysfs` ioctl. APatch and other non-KSU installations keep the LKM and try it by default after mounting ext4 staging. The bundled `.ko` files support aarch64 only. Automatic selection requires an exact kernel line and Android/GKI tag match; unknown combinations are rejected. Prebuilt LKMs must still be validated for ABI compatibility on the corresponding real device. If the device crashes during `insmod`, a persistent circuit-breaker marker prevents the LKM from loading again on the next boot while preserving the rest of Hybrid Mount. See [`module/lkm/README.md`](module/lkm/README.md) for the support matrix, checksums, sources, and licenses.

## VFS backend

VFS is Hybrid Mount's own kernel-side injection path, driven over the keyring by the `hybridmount` module. It is an independent implementation and does not interoperate with NoMount.

**How the provider is identified.** The boot decision keys on a read-only probe of the kernel key type `hybridmount`: if it answers with a supported version, the provider is usable. Separately, `vfs-doctor` classifies how it is present — an entry in `/proc/modules` means a loadable module registered it, a `/sys/module/hybridmount` directory without that entry means it is compiled into the kernel image, and if neither exists then no provider is present. Probing is read-only, so `status` and `vfs-doctor` never trigger an `insmod`.

**Boot logic.** If the key type answers with a supported version, the provider is bound and nothing is loaded. If no rule selects VFS, the bundled module is not loaded either. If a rule does select VFS while the probe is silent, the pipeline picks the bundled module matching the kernel line and Android/GKI tag exactly, loads it, and probes again; still unavailable, every `vfs` rule degrades to `ignore`, or fails the boot when `vfs_strict = true`. The load runs before the mount plan is built, because planning rewrites `vfs` rules to `ignore` while the provider is silent and the executor would then return early. A circuit-breaker marker is written before `insmod` and cleared when the attempt returns, so only a kernel crash leaves it behind; the next boot then refuses an automatic retry until the marker is removed by hand.

**Integrating VFS into a kernel.** Releases ship a prebuilt aarch64 module for every supported Android/GKI target and load it automatically, so those kernels need no integration step. Build it in when you want to avoid the `insmod`, or when your kernel line has no prebuilt. From the root of a kernel tree:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash
```

`--cleanup`:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash -s -- --cleanup
```

This copies the sources into `fs/hybridmount/` and adds them to `fs/Makefile` and `fs/Kconfig`; enable `CONFIG_HYBRIDMOUNT=y` to build it in or `=m` to build it as a module. `bash -s -- --cleanup` reverts every change. A tree that already integrates NoMount is refused: both implementations hijack inode operations and the kernel will not stop them coexisting, since they register different key types.

**Diagnosing.** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor` reports the presence state, the version the key type answered, the supported versions, and why a provider is unusable when it is.

## Feedback

Before installation or reporting an issue, read the [Usage Notice](USAGE_NOTICE.md). Include the KernelSU/APatch bugreport, module version, and reproduction steps. Contact us through [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) or the [Telegram group](https://t.me/hybridmountchat).

## Languages

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ES.md)
- [Français](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_FR.md)
- [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ID.md)
- [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_IT.md)
- [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_JA.md)
- [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_RU.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)
- [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_UK.md)
- [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_VI.md)
- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH.md)
- [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH_TW.md)

## Acknowledgements

- Thanks to [Anatdx](https://github.com/Anatdx)
- Thanks to [Tools-cx-app](https://github.com/Tools-cx-app)
- Thanks to [KernelSU](https://github.com/tiann/KernelSU)
- Thanks to [MKSU by 5ec1cff](https://github.com/5ec1cff/KernelSU)
- Thanks to [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- Thanks to [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- Thanks to [NoMount](https://github.com/maxsteeel/nomount)

## License

- Core (Rust and module scripts): GPL-3.0-only (see [`LICENSE`](LICENSE)).
- WebUI: Apache-2.0 (see [`webui/LICENSE`](webui/LICENSE)).
- Optional ext4 sysfs LKM (source and prebuilt `.ko` files): GPL-2.0-only, derived from [Mountify](https://github.com/backslashxx/mountify); see [`module/lkm/README.md`](module/lkm/README.md) and [`module/lkm/src/LICENSE`](module/lkm/src/LICENSE).
- VFS kernel subsystem (`hybridmount` module): GPL-2.0-only, a fork of [NoMount](https://github.com/maxsteeel/nomount); see [`module/vfs/README.md`](module/vfs/README.md), [`module/vfs/src/LICENSE`](module/vfs/src/LICENSE) and [THIRD_PARTY.md](THIRD_PARTY.md) for attribution and license details.
