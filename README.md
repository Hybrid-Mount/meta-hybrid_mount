# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/master/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)

Hybrid Mount is a mount orchestration metamodule for **KernelSU** and **APatch**.  
It merges module files into Android partitions with a hybrid strategy:

- **OverlayFS** when kernel/filesystem support is stable.
- **Magic Mount (bind mount)** as fallback or per-path override.

The runtime is designed for predictable boot behavior, conflict visibility, and policy-level control.

**[🇨🇳 中文文档](README_ZH.md)**

---

## Table of Contents

- [Design Goals](#design-goals)
- [Architecture](#architecture)
- [Repository Layout](#repository-layout)
- [Configuration](#configuration)
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

## Architecture

At startup, `hybrid-mount` follows this pipeline:

1. Load config (file + CLI override).
2. Scan module tree and inventory mountable entries.
3. Generate an execution plan (overlay/magic/ignore).
4. Apply mounts and persist runtime state.
5. Emit diagnostics/conflict reports when requested.

Key implementation modules:

- `src/conf`: config schema, loader, CLI handlers.
- `src/core/inventory`: module scanning and inventory modeling.
- `src/core/ops`: planning, execution, synchronization.
- `src/mount`: overlayfs + magic-mount backends.
- `src/sys`: filesystem/mount helpers and low-level integration.

## Repository Layout

```text
.
├─ src/                 # daemon/runtime implementation
├─ module/              # module scripts and packaging assets
├─ xtask/               # build/release automation commands
├─ tools/notify/        # optional helper binary
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
| `disable_umount` | bool | `false` | Skip unmount operations (debug-only). |
| `allow_umount_coexistence` | bool | `false` | Allow coexistence with existing umount behavior. |
| `enable_overlay_fallback` | bool | `false` | When overlayfs is unavailable, allow falling back to Magic Mount for planned overlay modules. |
| `default_mode` | `overlay` \| `magic` | `overlay` | Default policy for module paths. |
| `rules` | map | `{}` | Per-module path-level mount policy. |

### Example

```toml
moduledir = "/data/adb/modules"
mountsource = "KSU"
partitions = ["system", "vendor"]
overlay_mode = "ext4"
disable_umount = false
allow_umount_coexistence = false
enable_overlay_fallback = false
default_mode = "overlay"

[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/tool" = "overlay"
"vendor/lib64/libfoo.so" = "ignore"
```

## Policy Behavior Matrix

This matrix clarifies what happens under each policy and runtime condition:

| Rule result | OverlayFS available | `enable_overlay_fallback` | Effective behavior |
| --- | --- | --- | --- |
| `overlay` | yes | any | Mount with OverlayFS. |
| `overlay` | no | `false` | Skip mount and report as failed planning/execution item. |
| `overlay` | no | `true` | Retry as Magic Mount (bind mount). |
| `magic` | yes/no | any | Mount with Magic Mount directly. |
| `ignore` | yes/no | any | Do not mount this path. |

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
```

Artifacts are produced under `output/`.

## Operational Notes

- If a bad config causes boot issues, regenerate a minimal config with `gen-config` and reapply module rules incrementally.
- For binary size optimization, prefer dependency feature trimming and release profile tuning before invasive refactors.

## License

Licensed under [GPL-3.0](LICENSE).
