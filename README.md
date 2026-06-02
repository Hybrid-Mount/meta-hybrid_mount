# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-Apache--2.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount is a mount orchestration metamodule for **KernelSU** and **APatch**.
It merges module files into Android partitions through a unified policy engine backed by three mount backends:

- **OverlayFS** — layered mounts for broad compatibility.
- **Magic Mount** — bind-mount for direct path replacement or fallback.
- **Kasumi** — LKM-backed routing with runtime hide, spoof, and stealth features.

A built-in **SolidJS WebUI** provides graphical management, live state monitoring, and configuration editing.

Releases are published in three flavors — see [Build Flavors](#build-flavors) for a detailed comparison. Unless noted otherwise, the rest of this README describes the `full` build.

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)**

---

## Table of Contents

- [Features](#features)
- [Build Flavors](#build-flavors)
- [Quick Start](#quick-start)
- [Mount Modes](#mount-modes)
- [WebUI](#webui)
- [Language Support](#language-support)
- [Configuration](#configuration)
- [Kasumi](#kasumi)
- [Policy Reference](#policy-reference)
- [CLI](#cli)
- [Architecture](#architecture)
- [Build](#build)
- [Operational Notes](#operational-notes)
- [License](#license)

---

## Build Flavors

Hybrid Mount is released in three flavors, each targeting a different use case:

| Flavor | Binary | WebUI | Daemon / CLI | Kasumi LKM | Use case |
|--------|--------|-------|-------------|------------|----------|
| **Full** | Yes | Yes | Yes | Yes | Users who need Kasumi-backed routing or hide/spoof capabilities. |
| **Lite** | Yes | Yes | Yes | No | Users who want the WebUI and full policy engine but don't need LKM-backed stealth features. |
| **Nano** | Yes | No | No | No | Minimalists who just want mount orchestration via config file — no runtime daemon, no WebUI, no CLI. |

### Full

The `full` flavor includes all supported mount backends (OverlayFS, Magic Mount, Kasumi), the SolidJS WebUI, the Unix-socket daemon with HTTP/SSE, the CLI, and the Kasumi LKM assets. Use Full when Kasumi-backed routing or auxiliary hide/spoof features are required. Built with Cargo features `kasumi` (which implies `control-plane`).

### Lite

The `lite` flavor strips the Kasumi LKM and all Kasumi-related features (hide, spoof, stealth, kstat rules, uname spoofing, etc.) but keeps the WebUI, daemon, CLI, and both OverlayFS and Magic Mount backends. Choose Lite if:

- Your kernel doesn't support loading external LKMs.
- You don't need runtime hide/spoof capabilities.
- You want a smaller download while keeping the WebUI and daemon management interface.

Lite builds use the feature set `control-plane` only (`--no-default-features --features control-plane`). The WebUI's Kasumi panel is hidden automatically.

### Nano

The `nano` flavor is a **config-only** build (`--no-default-features` — no Cargo features enabled). It strips the WebUI, daemon, CLI, and all control-plane infrastructure. What remains is a minimal binary that reads `config.toml`, generates a mount plan, and executes it — then exits. Key characteristics:

- **No runtime daemon** — no background process, no socket, no WebUI, no CLI subcommands.
- **No WebUI** — the `webroot/`, `launcher.png`, and `service.sh` assets are removed from the package.
- **Mount-only operation** — the binary runs during boot, mounts everything according to the config, and terminates.
- **Default mode is `magic`** — Nano ships with `default_mode = "magic"` pre-set in its config, preferring bind mounts when no daemon is available to manage ext4 images.
- **Module mode markers** — install-time volume-key selection writes an empty `overlay` or `magic` marker in each managed module root, and Nano reads that instead of a whitelist. Marker filenames are matched case-insensitively.
- **No resident Hybrid Mount process** — after boot-time mounting completes, the Nano binary exits.

Choose Nano if you want predictable, daemon-free mount orchestration with a smaller runtime surface.

### Feature matrix

| Feature | Full | Lite | Nano |
|---------|------|------|------|
| OverlayFS backend | Yes | Yes | Marker-based |
| Magic Mount backend | Yes | Yes | Yes (default) |
| Kasumi backend | Yes | No | No |
| WebUI | Yes | Yes | No |
| CLI (`hybrid-mount` subcommands) | Yes | Yes | No |
| Daemon (Unix + TCP/SSE) | Yes | Yes | No |
| Config caching & runtime apply | Yes | Yes | No |
| Kasumi hide/spoof/stealth | Yes | No | No |
| LKM autoload | Yes | No | No |
| Cargo features | `kasumi` (implies `control-plane`) | `control-plane` only | none |
| ZIP size (approx.) | ~4 MB | ~2 MB | ~1 MB |

## Features

- **Three backends, one policy engine** — assign paths to OverlayFS, Magic Mount, or Kasumi with per-path granularity.
- **Deterministic planning** — conflicts are detected at plan time, not discovered randomly at boot.
- **Built-in WebUI** — manage modules, edit configuration, monitor runtime state, and control Kasumi features in full builds.
- **Kasumi runtime integration** — LKM autoload, mirror routing, mount hiding, maps/statfs spoofing, UID hiding, uname spoofing, and kstat rules.
- **Config caching** — runtime config cache with incremental patching and immediate apply support.
- **Recovery-friendly** — stale runtime files are cleaned automatically; misconfigurations can be reset via `api config-reset`.
- **Automation-friendly** — JSON-over-Unix-socket daemon protocol + HTTP API for scripting or external controllers.

---

## Quick Start

### Installation

1. Install [KernelSU](https://kernelsu.org/) or [APatch](https://apatch.dev/) on your device.
2. Download the latest Hybrid Mount `full`, `lite`, or `nano` release ZIP from [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases).
3. Flash the ZIP through your root manager's module installer.
4. Reboot. Hybrid Mount will auto-detect your environment and apply the default overlay policy.

### Post-install

```bash
# Check runtime status
hybrid-mount daemon status

# List detected modules
hybrid-mount api modules-list
```

To access the WebUI (Full/Lite flavors), open your root manager app (KernelSU or APatch), find Hybrid Mount in the modules list, and tap it — the manager will launch the WebUI in an embedded WebView.

### Changing mount mode for a module

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## Mount Modes

| Mode | Backend | Best for |
|------|---------|----------|
| `overlay` | OverlayFS | Modules that add or replace files without conflicts. Default mode. |
| `magic` | Bind mount | Modules that need direct per-file replacement. |
| `kasumi` | Kasumi LKM | Modules requiring explicit mirror routing or runtime hide/spoof features. |
| `ignore` | — | Excluding specific paths from any mount processing. |

### OverlayFS storage modes

The OverlayFS backend supports two storage strategies for the upper/work layers:

- `ext4` (default) — creates an ext4 disk image. Persists across reboots, supports xattr.
- `tmpfs` — uses a tmpfs mount. Volatile, lighter weight, but lost on reboot.

```toml
overlay_mode = "ext4"
```

---

## WebUI

Hybrid Mount includes a **SolidJS-based WebUI** served by the daemon over a local TCP socket (HTTP/SSE). CLI and automation clients communicate over a Unix socket. The daemon prints the WebUI access URL to logcat on startup.

The WebUI is designed to be opened directly from your **root manager app** (KernelSU or APatch manager) — tap the module entry and the manager will launch the WebUI in an embedded WebView. No external browser is required on-device.

### Capabilities

- **Status dashboard** — live mount statistics, active partitions, storage mode, daemon health.
- **Module management** — list all detected modules with their effective mount modes; apply mode changes interactively.
- **Configuration editor** — full config.toml editing with validation, including per-module path rules.
- **Kasumi control panel** — LKM status, rule listing, feature toggles, uname configuration, maps/kstat rules (Full flavor only).

### Language Support

The WebUI currently ships with these locales:

- English (`en-US`, default)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

README documentation is available in [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md), [Simplified Chinese](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md), [Traditional Chinese](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md), [Japanese](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md), [Spanish](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md), [Italian](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md), [Russian](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md), [Ukrainian](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md), and [Vietnamese](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md).

### Access

The WebUI runs on `http://127.0.0.1:<random-port>` with a cryptographic access token. The daemon manages the lifecycle — no separate web server needed. On-device, open through your root manager's WebView; remotely, forward the port via ADB.

---

## Configuration

Default path: `/data/adb/hybrid-mount/config.toml`.

### Top-level fields

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Module source directory. |
| `mountsource` | string | auto-detect | Runtime source tag (`KSU`, `APatch`). |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Overlay upper/work storage mode. |
| `disable_umount` | bool | `false` | Skip umount operations (debug only). |
| `default_mode` | `overlay` \| `magic` \| `kasumi` | `overlay` | Global default mount policy. |
| `daemon_startup_mode` | `on-demand` \| `persistent` | `on-demand` | Daemon startup behavior. |
| `rules` | map | `{}` | Per-module and per-path mount policies. |

### Example

```toml
moduledir = "/data/adb/modules"
overlay_mode = "ext4"
default_mode = "overlay"
daemon_startup_mode = "on-demand"

[rules.viper4android]
default_mode = "magic"

[rules.viper4android.paths]
"system/etc/audio_policy.conf" = "overlay"

[rules.sensitive_module]
default_mode = "kasumi"

[rules.sensitive_module.paths]
"system/bin/helper" = "kasumi"
"system/etc/placeholder" = "ignore"
```

---

## Kasumi

Kasumi is the **LKM-backed** backend. Beyond mount routing, it provides a suite of runtime hide and spoof capabilities.

### Activation

Setting `kasumi.enabled = true` makes the backend available. The Kasumi runtime is actually enabled when at least one of these conditions is met:

- The mount plan contains a Kasumi-managed module or path.
- An auxiliary feature is configured (hidexattr, mount hide, maps spoof, statfs spoof, UID hiding, uname spoof, cmdline replacement, kstat rules, or user hide rules).

### Key config fields

| Field | Purpose |
| --- | --- |
| `kasumi.enabled` | Master switch for Kasumi integration. |
| `kasumi.lkm_autoload` | Auto-load the Kasumi LKM during startup. |
| `kasumi.lkm_dir` | LKM search directory. |
| `kasumi.lkm_kmi_override` | Optional KMI version override for LKM selection. |
| `kasumi.mirror_path` | Mirror root used by Kasumi rules (default `/dev/kasumi_mirror`). |
| `kasumi.enable_kernel_debug` | Toggle kernel-side debug logging. |
| `kasumi.enable_stealth` | Explicit stealth mode. |
| `kasumi.enable_hidexattr` | Compatibility umbrella — enables stealth, mount hide, maps spoof, and statfs spoof together. |
| `kasumi.enable_mount_hide` | Hide mounts globally or by path pattern. |
| `kasumi.mount_hide.path_pattern` | Path pattern for mount hiding. |
| `kasumi.enable_maps_spoof` | Enable `/proc/<pid>/maps` spoofing. |
| `kasumi.maps_rules` | Per-inode/device maps rewrite rules. |
| `kasumi.enable_statfs_spoof` | Enable `statfs` spoofing. |
| `kasumi.statfs_spoof.path` / `.spoof_f_type` | Path-scoped statfs spoof configuration. |
| `kasumi.hide_uids` | UIDs to hide from Kasumi-aware queries. |
| `kasumi.uname_mode` | Uname spoof mode: `scoped` (per-process) or `global`. |
| `kasumi.uname.*` | Structured uname spoof (sysname, nodename, release, version, machine, domainname). |
| `kasumi.cmdline_value` | Replacement `/proc/cmdline` content. |
| `kasumi.kstat_rules` | Per-target stat metadata spoof rules. |

### Commands

```bash
# Status and diagnostics
hybrid-mount kasumi status
hybrid-mount kasumi version
hybrid-mount kasumi features
hybrid-mount kasumi hooks
hybrid-mount kasumi list          # list active rules
hybrid-mount lkm status

# Runtime control
hybrid-mount kasumi apply-config-runtime
hybrid-mount kasumi clear
hybrid-mount kasumi release-connection
hybrid-mount kasumi invalidate-cache
hybrid-mount kasumi fix-mounts

# Uname spoofing (scoped or global)
hybrid-mount kasumi set-uname --mode scoped <release> <version>
hybrid-mount kasumi clear-uname --mode scoped
hybrid-mount kasumi restore-uname-global

# Rule management
hybrid-mount kasumi rule add --target /system/bin/tool --source /data/adb/modules/my_module/system/bin/tool
hybrid-mount kasumi rule merge --target /system/lib64 --source /data/adb/modules/my_module/system/lib64
hybrid-mount kasumi rule hide --path /system/bin/su
hybrid-mount kasumi rule delete --path /system/bin/old_tool
hybrid-mount kasumi rule add-dir --target-base /system/lib64 --source-dir /data/adb/modules/my_module/system/lib64
hybrid-mount kasumi rule remove-dir --target-base /system/lib64 --source-dir /data/adb/modules/my_module/system/lib64
```

---

## Policy Reference

### Precedence

When multiple policies could apply to a path, evaluation order is:

1. **Path-level override** — `rules.<module>.paths["<path>"]`
2. **Module-level default** — `rules.<module>.default_mode`
3. **Global default** — `default_mode`

### Behavior matrix

| Rule result | Backend available? | Effective behavior |
| --- | --- | --- |
| `overlay` | Yes | Mount with OverlayFS. |
| `overlay` | No | Skip and report as failed. |
| `magic` | n/a | Mount with Magic Mount. |
| `kasumi` | Yes | Route through Kasumi. |
| `kasumi` | No | Skip Kasumi mapping. |
| `ignore` | n/a | Do not mount. |

### Module marker files

Hybrid Mount also recognizes marker files in module directories. These markers are expected to be regular files; only the filename is used. Marker filenames are matched case-insensitively for ASCII letters, so `DISABLE`, `Disable`, and `disable` are treated as the same marker.

| Marker | Location | Effect |
| --- | --- | --- |
| `disable` | Module root | Excludes the module from mount planning and reports it as disabled. |
| `remove` | Module root | Excludes the module from mount planning; normally created by the root manager during removal. |
| `skip_mount` | Module root | Excludes the module from mount processing and records it in the runtime skip list. |
| `mount_error` | Module root | Marks a module that was skipped after a mount failure. Recovery and daemon commands may create or clear it. |
| `overlay` / `magic` | Module root, Nano builds | Selects the module default mount backend for Nano builds. Full and Lite builds use config rules instead. |
| `.replace` | Inside a module directory | Applies replacement semantics to the containing directory. The marker itself is not copied as normal module content; prepared overlay layers preserve the directory and set overlay opaque metadata where supported. |

If multiple case variants of the same marker exist in one directory, cleanup operations remove all matching variants.

### Practical recipes

- **One problematic binary on bind mount, rest on overlay**: set module default to `overlay`, override the binary path to `magic`.
- **Temporarily exclude a conflicting file**: set the path to `ignore`.

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

### Global options

| Flag | Description |
| ---- | ----------- |
| `-c, --config <PATH>` | Custom config file path. |

### Subcommands

| Command | Description |
| ------- | ----------- |
| `gen-config` | Generate a default config file. |
| `logs` | Print recent daemon logs. |
| `api storage` | Query storage mode (ext4/tmpfs). |
| `api mount-stats` | Print mount statistics. |
| `api mount-topology` | Print mount topology tree. |
| `api partitions` | List managed partitions. |
| `api system-info` | Print system information. |
| `api version` | Print daemon version. |
| `api config-get` | Print effective config as JSON. |
| `api config-set --config <JSON>` | Replace full config. |
| `api config-patch --patch <JSON>` | Merge patch into config. |
| `api config-reset` | Reset config to defaults. |
| `api modules-list` | List detected modules. |
| `api modules-apply --modules <JSON>` | Apply module mode changes. |
| `api lkm` | Query LKM status. |
| `api features` | List supported features. |
| `api hooks` | List Kasumi hooks status. |
| `api kernel-uname` | Print kernel uname. |
| `api open-url --url <URL>` | Open URL on device. |
| `api reboot` | Reboot the device. |
| `api kasumi-maps-add --rule <JSON>` | Add a Kasumi maps spoof rule. |
| `api kasumi-maps-clear` | Clear all Kasumi maps spoof rules. |
| `daemon launch` | Start daemon in foreground. |
| `daemon serve` | Start daemon (service mode). |
| `daemon ping` | Check daemon liveness. |
| `daemon webui-start` | Start WebUI only. |
| `daemon stop` | Stop the daemon. |
| `daemon status` | Query daemon runtime state. |
| `kasumi ...` | Kasumi management (see [Kasumi](#kasumi)). |
| `lkm load / unload / status` | LKM lifecycle management. |
| `hide list / add / remove / apply` | User hide rule management. |

---

## Architecture

```text
┌─────────────────────────────────────────────┐
│                  config.toml                  │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              Inventory Discovery              │
│         Scan module tree, classify entries    │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              Mount Planner                    │
│    Evaluate rules (path > module > global)    │
│    Generate overlay / magic / kasumi plan     │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              Executors                        │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐ │
│  │ OverlayFS│ │  Magic   │ │   Kasumi     │ │
│  │ executor │ │  Mount   │ │   executor   │ │
│  └──────────┘ └──────────┘ └──────────────┘ │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│            Runtime State + Daemon             │
│   Persist state → Unix socket → WebUI/CLI     │
└─────────────────────────────────────────────┘
```

The executor is driven by a **typestate state machine** (`src/core/controller.rs`): `MountController<Init> → StorageReady → Planned → Executed`. Each transition represents one pipeline stage, ensuring the mount process is always in a well-defined state.

### Source layout

```text
src/
├── conf/          Config schema, TOML loader, CLI definition, handlers
├── domain/        Core types: MountMode, ModuleRules, path matching
├── partitions/    Managed partition auto-discovery
├── core/
│   ├── inventory/ Module discovery and listing
│   ├── ops/       Mount plan generation and per-backend execution
│   ├── daemon/    Unix + TCP dual-protocol daemon (CLI + WebUI/SSE)
│   ├── api/       Payload builders for WebUI endpoints
│   ├── startup/   Boot sequence, recovery, retry logic
│   ├── storage/   Shared storage helpers (ext4 image, tmpfs)
│   └── runtime_state/ Daemon state persistence
├── mount/
│   ├── overlayfs/ OverlayFS backend (ext4 image / tmpfs)
│   ├── magic_mount/ Bind-mount backend
│   └── kasumi/    Kasumi rule compilation, runtime, status
├── sys/           Low-level: mount syscalls, LKM load/unload, Kasumi UAPI
└── utils/         Logging, path utilities, validation

webui/
├── src/
│   ├── routes/    Page components (Status, Config, Modules, Kasumi, Info)
│   ├── components/ Shared UI components (NavBar, Toast, Skeleton)
│   ├── lib/       API bridge, stores, codecs, i18n
│   └── locales/   9-language internationalization

xtask/             Build and release automation
module/            Module packaging scripts and static assets
```

---

## Build

### Prerequisites

- Rust nightly (from `rust-toolchain.toml`)
- Android NDK r27+ and `cargo-ndk`
- Node.js 20+ and pnpm (for WebUI)

### Commands

```bash
# Full release package (binary + WebUI + Kasumi) → output/
cargo run -p xtask -- build --release --flavor full

# Lite release package (binary + WebUI, no Kasumi) → output/
cargo run -p xtask -- build --release --flavor lite

# Nano release package (config-only, no WebUI/CLI/daemon) → output/
cargo run -p xtask -- build --release --flavor nano

# Binary only (skip WebUI)
cargo run -p xtask -- build --release --skip-webui

# Local arm64 debug build
./scripts/build-local.sh

# Local lite debug build
./scripts/build-local.sh --lite

# Local nano debug build
./scripts/build-local.sh --nano

# Local build with prebuilt Kasumi LKM .ko assets (full only)
./scripts/build-local.sh --release --kasumi-lkm-dir /path/to/kasumi-lkm

# WebUI dev server (hot reload)
cd webui && pnpm install && pnpm dev

# Lint everything
cargo run -p xtask -- lint
cd webui && pnpm lint

# Run tests
cargo +nightly test
cd webui && pnpm test
```

### Release profile

The release profile uses `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip = true`, and `panic = "abort"` to reduce binary size.

### CI gates and feature flag linting

Every change must pass the following CI checks (defined in `.github/workflows/`):

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings` (warnings are errors)
- `cargo test --all-targets --workspace`
- WebUI: `pnpm lint` + `pnpm test`
- License header check on all source files

`cargo clippy --all-features` (what `xtask lint` runs) only checks the `full` flavor. When making changes, also verify that the **lite** (`--no-default-features --features control-plane`) and **nano** (`--no-default-features`) flavor combinations compile. Code touching Kasumi must be behind `#[cfg(feature = "kasumi")]`; code touching the daemon/CLI/WebUI API must be behind `#[cfg(feature = "control-plane")]`.

---

## Operational Notes

- **Mount source auto-detection**: fresh installs detect the runtime environment automatically. Only set `mountsource` explicitly if auto-detection fails.
- **Recovery from bad config**: run `hybrid-mount api config-reset` to reset to defaults, then reapply rules incrementally. Use `gen-config` to regenerate a fresh config file.
- **Config caching**: the runtime maintains a cached config. Use `api config-patch --apply-runtime` to apply changes immediately, or restart the daemon.
- **Kasumi LKM (full builds only)**: the LKM must match the running kernel. Use `lkm_kmi_override` if the auto-detected KMI is incorrect.
- **`kasumi clear`**: clears runtime state and releases kernel connection. Existing kernel-side rules may persist until LKM reload.
- **Binary size**: prefer dependency feature trimming and profile tuning before invasive refactoring.

---

## License

Licensed under [Apache-2.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE).
