# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-Apache--2.0-blue?style=flat-square)

Hybrid Mount is a mount orchestration metamodule for **KernelSU** and **APatch**.  
It merges module files into Android partitions with three mount modes:

- **OverlayFS** for compatibility-first layered mounts.
- **Magic Mount (bind mount)** for direct path binding or fallback.
- **Kasumi** for explicit Kasumi routing and runtime-backed hide/spoof features.

The runtime is designed for predictable boot behavior, conflict visibility, and policy-level control.

**[🇨🇳 中文文档](README_ZH.md)**

---

## Table of Contents

- [Design Goals](#design-goals)
- [Mount Modes](#mount-modes)
- [Architecture](#architecture)
- [Repository Layout](#repository-layout)
- [Configuration](#configuration)
- [Kasumi](#kasumi)
- [Policy Behavior Matrix](#policy-behavior-matrix)
- [CLI](#cli)
- [Build](#build)
- [Operational Notes](#operational-notes)
- [License](#license)

---

## Design Goals

1. **Compatibility-first mounting** across diverse Android kernels.
2. **Deterministic behavior** through explicit planning and conflict analysis.
3. **Operational safety** with recovery-friendly defaults.
4. **Automation-friendly CLI** for WebUI or external controllers.

## Mount Modes

Hybrid Mount currently supports three backend strategies:

- `overlay`: use OverlayFS for module paths that can be merged safely.
- `magic`: use Magic Mount bind mounts for direct per-path replacement or fallback.
- `kasumi`: route module paths through the Kasumi mirror/runtime when the module or path explicitly requires it.

## Architecture

At startup, `hybrid-mount` follows this pipeline:

1. Load config (file + CLI override).
2. Scan module tree and inventory mountable entries.
3. Generate an execution plan (overlay/magic/kasumi/ignore).
4. Apply mounts and persist runtime state.
5. Emit diagnostics/conflict reports when requested.

Key implementation modules:

- `src/conf`: config schema, loader, CLI handlers.
- `src/core/inventory`: module scanning and inventory modeling.
- `src/core/ops`: planning, execution, synchronization.
- `src/mount`: OverlayFS, Magic Mount, and Kasumi backends.
- `src/sys`: filesystem/mount helpers and low-level integration.

## Repository Layout

```text
.
├─ src/                 # daemon/runtime implementation
├─ module/              # module scripts and packaging assets
├─ xtask/               # build/release automation commands
├─ Cargo.toml           # workspace + runtime crate settings
└─ README*.md           # user and developer docs
```

## Configuration

Default path: `/data/adb/hybrid-mount/config.toml`.

### Top-level fields

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Module source directory. |
| `mountsource` | string | auto-detect | Runtime source tag (e.g. `KSU`, `APatch`). |
| `partitions` | list\|csv string | `[]` | Extra managed partitions. |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Overlay upper/work backing mode. |
| `disable_umount` | bool | `false` | Skip umount operations (debug-only). |
| `enable_overlay_fallback` | bool | `false` | When overlayfs is unavailable, allow falling back to Magic Mount for planned overlay modules. |
| `default_mode` | `overlay` \| `magic` \| `kasumi` | `overlay` | Default policy for module paths. |
| `rules` | map | `{}` | Per-module path-level mount policy. |

### Example

```toml
moduledir = "/data/adb/modules"
mountsource = "KSU"
partitions = ["system", "vendor"]
overlay_mode = "ext4"
disable_umount = false
enable_overlay_fallback = false
default_mode = "overlay"

[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/tool" = "overlay"
"vendor/lib64/libfoo.so" = "ignore"
```

## Kasumi

`Kasumi` is the third mount backend in Hybrid Mount. It is kernel/LKM-backed and is used when a module/path is explicitly routed to `kasumi`, or when Kasumi-only runtime features are required.

It currently covers two categories of work:

- `mode = "kasumi"` mount mapping for modules or paths that should resolve from the Kasumi mirror tree.
- Auxiliary runtime features such as stealth/hide-xattr behavior, mount hiding, `/proc/<pid>/maps` spoofing, `statfs` spoofing, UID hiding, uname/cmdline spoofing, and per-file kstat spoof rules.

### When runtime turns on

Setting `kasumi.enabled = true` only makes the backend available. Hybrid Mount actually enables the Kasumi runtime when at least one of these is true:

- the generated mount plan contains at least one Kasumi-managed module/path
- an auxiliary feature is configured (`enable_hidexattr`, `enable_mount_hide`, `enable_maps_spoof`, `enable_statfs_spoof`, `hide_uids`, `cmdline_value`, `uname*`, `maps_rules`, `kstat_rules`, or persisted user hide rules)

Behavior details that matter in practice:

- `enable_hidexattr` is a compatibility umbrella and effectively turns on `stealth`, `mount_hide`, `maps_spoof`, and `statfs_spoof`
- `mount_hide.path_pattern` and `statfs_spoof.{path,spoof_f_type}` also count as enabling those features
- the CLI disable commands now clear those subordinate structured fields so `disable` really disables the feature instead of leaving it implicitly active

### Key config fields

| Key | Purpose |
| --- | --- |
| `kasumi.enabled` | Master switch for Kasumi integration. |
| `kasumi.lkm_autoload` | Try to auto-load the Kasumi LKM during startup. |
| `kasumi.lkm_dir` / `kasumi.lkm_kmi_override` | LKM search directory and optional KMI override. |
| `kasumi.mirror_path` | Runtime mirror root used by Kasumi rules, default `/dev/kasumi_mirror`. |
| `kasumi.enable_kernel_debug` | Toggle kernel-side debug output. |
| `kasumi.enable_stealth` | Explicit stealth mode toggle. |
| `kasumi.enable_hidexattr` | Compatibility umbrella for stealth + hide/spoof helpers. |
| `kasumi.enable_mount_hide` / `kasumi.mount_hide.path_pattern` | Hide mounts globally or with a path pattern. |
| `kasumi.enable_maps_spoof` / `kasumi.maps_rules` | Enable maps spoofing and install inode/device rewrite rules. |
| `kasumi.enable_statfs_spoof` / `kasumi.statfs_spoof.*` | Enable generic or path-scoped `statfs` spoofing. |
| `kasumi.hide_uids` | Hide selected UIDs from Kasumi-aware queries. |
| `kasumi.uname.*` | Structured uname spoof fields. |
| `kasumi.cmdline_value` | Replacement kernel cmdline payload. |
| `kasumi.kstat_rules` | Per-target stat metadata spoof rules. |

### Example

```toml
[kasumi]
enabled = true
lkm_autoload = true
mirror_path = "/dev/kasumi_mirror"
enable_mount_hide = true

[rules.my_module]
default_mode = "kasumi"

[rules.my_module.paths]
"system/bin/su" = "kasumi"
```

### Useful commands

```bash
# runtime/LKM status
hybrid-mount kasumi status
hybrid-mount kasumi version
hybrid-mount kasumi features
hybrid-mount lkm status

# enable/disable runtime-backed features
hybrid-mount kasumi enable
hybrid-mount kasumi disable
hybrid-mount kasumi mount-hide enable --path-pattern /dev/kasumi_mirror
hybrid-mount kasumi statfs-spoof enable --path /system --f-type 0x794c7630
hybrid-mount kasumi maps add --target-ino 1 --target-dev 2 --spoofed-ino 3 --spoofed-dev 4 --path /dev/kasumi_mirror/system/bin/sh
hybrid-mount kasumi kstat upsert --target-ino 11 --target-path /system/bin/app_process64 --spoofed-ino 22 --spoofed-dev 33
```

Operational caveat:

- `kasumi kstat clear-config` only removes persisted config. Existing kernel kstat spoof rules may remain until the Kasumi LKM is reloaded or the whole runtime is rebuilt.

## Policy Behavior Matrix

This matrix clarifies what happens under each policy and runtime condition:

| Rule result | Backend availability | `enable_overlay_fallback` | Effective behavior |
| --- | --- | --- | --- |
| `overlay` | OverlayFS available | any | Mount with OverlayFS. |
| `overlay` | OverlayFS unavailable | `false` | Skip mount and report as failed planning/execution item. |
| `overlay` | OverlayFS unavailable | `true` | Retry as Magic Mount (bind mount). |
| `magic` | n/a | any | Mount with Magic Mount directly. |
| `kasumi` | Kasumi available | any | Mount with Kasumi directly. |
| `kasumi` | Kasumi unavailable or disabled | any | Skip Kasumi mapping for this path/module. |
| `ignore` | n/a | any | Do not mount this path. |

### Rule precedence

When multiple policies may apply, evaluation follows this order:

1. Path-level override (`rules.<module>.paths["..."]`)
2. Module-level default (`rules.<module>.default_mode`)
3. Global default (`default_mode`)

### Practical examples

- Keep one problematic binary on bind mount while the rest of the module uses overlay:
  - set module default to `overlay`
  - set `rules.<module>.paths["system/bin/<tool>"] = "magic"`
- Temporarily disable one conflicting file without disabling the full module:
  - set `rules.<module>.paths["..."] = "ignore"`
- For kernels with unstable OverlayFS support:
  - set `enable_overlay_fallback = true` to reduce boot-time mount failures.

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

Global options:

- `-c, --config <PATH>` custom config path
- `-m, --moduledir <PATH>` override module directory
- `-s, --mountsource <SOURCE>` override source tag
- `-p, --partitions <CSV>` override partition list

Subcommands:

- `gen-config` generate config file
- `show-config` print effective config JSON
- `save-config --payload <HEX_JSON>` save config from WebUI payload
- `save-module-rules --module <ID> --payload <HEX_JSON>` update one module rule set
- `modules` list detected modules

## Build

Prerequisites:

- Rust toolchain from `rust-toolchain.toml`
- Android NDK (recommended r27+)
- Node.js 20+ (only when building WebUI assets)

Build commands:

```bash
# full package
cargo run -p xtask -- build --release

# runtime only (skip web assets)
cargo run -p xtask -- build --release --skip-webui

# local arm64 debug package
./scripts/build-local.sh

# local package with prebuilt Kasumi LKM assets
./scripts/build-local.sh --release --kasumi-lkm-dir /path/to/kasumi-lkm
```

Artifacts are produced under `output/`.

## Operational Notes

- Fresh installs now rely on mount-source auto-detection unless `mountsource` is explicitly set in `config.toml`.
- If a bad config causes boot issues, regenerate a minimal config with `gen-config` and reapply module rules incrementally.
- For binary size optimization, prefer dependency feature trimming and release profile tuning before invasive refactors.

## License

Licensed under [Apache-2.0](LICENSE).
