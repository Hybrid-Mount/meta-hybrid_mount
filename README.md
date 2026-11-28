# Hybrid Mount

[AtomGit](https://atomgit.com/Hybrid-Mount/meta-hybrid_mount) · [GitHub](https://github.com/Hybrid-Mount/meta-hybrid_mount) · [简体中文](docs/README_ZH.md) · [Contributing](CONTRIBUTING.md)

Hybrid Mount is a KernelSU and APatch meta-module. At boot it scans module files and applies OverlayFS, Magic Mount or kernel-side VFS rules. Module source directories stay read-only.

## Installation

Read the [usage notice](USAGE_NOTICE.md), then install the ZIP from [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) in the KernelSU or APatch manager. Use the volume keys to choose a default backend on first install. Upgrades preserve your configuration.

## Configuration

Edit `/data/adb/hybrid-mount/config.toml` and reboot. Path rules override module defaults, which override the global default. Use `ignore` in a module or path rule to skip content. For example, this routes one module through Magic Mount and its hosts file through OverlayFS.

```toml
[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

See the [default configuration](module/config.toml) for storage and partition options. VFS requires a compatible kernel provider. When unavailable, VFS rules are skipped unless `vfs_strict = true` makes the mount run fail.

For custom mounts, place Lua 5.5.1 scripts in `/data/adb/hybrid-mount/lua/` and reboot. API 1 supplies `bind_mount`, `overlay_mount` and `vfs_mount` in the optional `before()` and `after()` callbacks. In API 2, call `change_mount(path, mode)` inside `load()` to route a module-relative file or subtree to `vfs`, `overlay` or `magic`. [Lua examples and API reference](docs/CUSTOM_MOUNT.md#english-reference)

- [Runtime mounting and recovery](docs/RUNTIME.md)
- [VFS CLI and diagnostics](docs/VFS_CLI.md) · [VFS kernel integration](module/vfs/README.md)
- [ext4 compatibility module](module/lkm/README.md)
- [Bug reports and contributions](CONTRIBUTING.md)

## Languages

[English](README.md) · [Español](docs/README_ES.md) · [Français](docs/README_FR.md) · [Bahasa Indonesia](docs/README_ID.md) · [Italiano](docs/README_IT.md) · [日本語](docs/README_JA.md) · [Русский](docs/README_RU.md) · [Türkçe](docs/README_TR.md) · [Українська](docs/README_UK.md) · [Tiếng Việt](docs/README_VI.md) · [简体中文](docs/README_ZH.md) · [繁體中文](docs/README_ZH_TW.md)

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
