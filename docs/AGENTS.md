# AGENTS.md

Guidance for AI coding agents working in this repo. Read this **before** editing or running anything. For user-facing feature docs, see `../README.md` (also `README_ZH.md` / `README_JP.md`). This file covers the things that are easy to get wrong.

## What this project is

**Hybrid Mount** (`hybrid-mount`) is a **mount-orchestration metamodule for KernelSU / APatch** (Android root frameworks). It runs as **root, during boot**, and merges other modules' files into read-only Android system partitions using two backends:

- **OverlayFS** — layered mounts (default).
- **Magic Mount** — bind mounts; also the fallback when OverlayFS is unavailable.

It is a **Rust binary** (`src/`) plus a **SolidJS WebUI** (`webui/`), packaged with shell scripts (`module/`) into a flashable ZIP by the build tool (`xtask/`).

## Hard constraints — read first

1. **This only builds for `aarch64-linux-android`.** There is no host-platform build. Building/running on Windows or x86 Linux is **not** a supported workflow.
   - The vast majority of real logic is gated behind `#[cfg(any(target_os = "linux", target_os = "android"))]` and calls Linux mount syscalls (`rustix`, `libc`, `procfs`, `loopdev`, `ksu`). It cannot execute on a normal dev host.
   - Building requires: **Rust nightly** (pinned by `rust-toolchain.toml`), **Android NDK r27+**, and **`cargo-ndk`**. WebUI needs **Node 20+** and **pnpm**.
   - **Do not assume you can `cargo check`/`cargo run` to validate.** On a host without the NDK/Android target it will fail. Validate by reading carefully + relying on CI, or build inside the proper NDK environment. State explicitly when you could not build/test.

2. **You cannot meaningfully run the module.** It mounts partitions on a booted, rooted Android device. There is no local "run the app" loop. The pure-logic unit tests (e.g. `src/domain/mod.rs`, `xtask`) are the only thing runnable off-device.

## Feature flags — the #1 thing agents break

There are **two build flavors**, expressed as Cargo feature combinations. **Any change must keep both compiling.**

| Flavor | Cargo features | What's included |
|--------|----------------|-----------------|
| **lite** (default) | `control-plane` | WebUI + daemon + CLI + OverlayFS/Magic |
| **nano** | none (`--no-default-features`) | Mount-only binary; **no daemon, no CLI, no WebUI** |

Consequences when editing Rust:

- Code touching the daemon / CLI / WebUI API must be behind `#[cfg(feature = "control-plane")]`.
- `main.rs` branches: with `control-plane` it parses CLI (`core::entry::run`); without it, it runs `core::startup::run_default()` (mount once and exit).
- When you add a `#[cfg(feature = ...)]` block, make sure the **non-feature path still compiles** (provide the `#[cfg(not(...))]` counterpart where a value is needed — see `core/ops/executor/mod.rs` for the pattern).
- `cargo clippy --all-features` (what `xtask lint` runs) won't catch a flavor that *removes* features. Mentally check the `nano` (no-default-features) build too.

## Architecture / mental model

Boot pipeline (`module/metamount.sh` → the binary → `core::startup`):

```
config.toml ─► Inventory (scan modules) ─► Planner (apply rules) ─► Executor (overlay/magic) ─► Finalize (persist state, cleanup)
```

The executor is driven by a **typestate state machine**: `MountController<Init> → StorageReady → Planned → Executed` in `src/core/controller.rs`. Each transition is one pipeline stage.

Source layout (`src/`):

- `conf/` — config schema (`schema.rs`), TOML loader, `clap` CLI definition (`cli.rs`), CLI handlers.
- `domain/` — core enums: `MountMode` (overlay/magic/ignore), `ModuleRules`, the **rule-resolution logic** (path override → module default → global default). Has the only substantial unit tests. Legacy `kasumi` config values are deserialized as magic mount for backward compatibility.
- `partitions.rs`, `defs.rs` — managed partition lists and **all the hardcoded `/data/adb/...` paths**. Treat these constants as load-bearing.
- `core/` — `inventory/` (module discovery), `ops/` (plan + per-backend executors), `daemon/` (Unix socket for CLI + TCP/SSE HTTP for WebUI), `api/` (WebUI payload builders), `startup/` (boot + recovery/retry), `storage/` (ext4 image / tmpfs), `runtime_state.rs`, `module_status.rs`, `recovery.rs`, `late_load.rs`.
- `mount/` — the two backends: `overlayfs/`, `magic_mount/`, plus `node.rs` (mount tree) and `umount_mgr.rs`.
- `sys/` — low-level syscalls: `mount.rs`, filesystem helpers, `nuke.rs`.
- `utils/` — logging, path helpers, validation.

`webui/` is a standalone SolidJS app (Vite, TypeScript, Material Web). `webui/src/lib/constants_gen.ts` is **generated** at build time — don't hand-edit it.

`xtask/` is the build/release/lint orchestrator (`cargo xtask ...`). It compiles via `cargo-ndk`, builds the WebUI, prunes assets per flavor, rewrites `config.toml` per flavor, and zips the package into `output/`.

## Build / lint / test

```bash
# Package builds (output/*.zip) — need NDK + cargo-ndk
cargo run -p xtask -- build --release --flavor lite   # or nano
cargo run -p xtask -- build --release --skip-webui    # binary only

# Local debug helper (arm64)
./scripts/build-local.sh            # lite debug
./scripts/build-local.sh --nano     # nano

# Full local verification (Rust fmt/clippy/tests + WebUI lint/test)
cargo run -p xtask -- lint

# Tests
cargo +nightly test --all-targets --workspace
cd webui && pnpm test

# WebUI dev server
cd webui && pnpm install && pnpm dev
```

**CI gates** (`.github/workflows/lints.yml` + `license_header.yml` + `dependency-audit.yml`) — your change must pass all of these:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings` (warnings are errors)
- `cargo test --all-targets --workspace`
- `cargo test -p hybrid-mount --no-default-features --features control-plane --lib`
- `cargo test -p hybrid-mount --no-default-features --lib`
- WebUI: `pnpm lint` + `pnpm test`
- ShellCheck on module/installer scripts, ZIP content verification, and license header checks.

## Conventions (enforced)

- **License header**: every non-WebUI source file starts with the GPL-3.0-only header (template in `LICENCE_HEADER`); files under `webui/` keep the Apache-2.0 header (template in `webui/LICENSE_HEADER`). New files **must** include it or CI fails.
- **rustfmt** (`rustfmt.toml`): edition 2024, `imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`. Always run `cargo fmt` before finishing.
- **Clippy clean** under `-D warnings`. No new warnings.
- **Conventional commits** (`cliff.toml` drives the changelog): `feat:`, `fix:`, `refactor:`, `docs:`, `style:`, `chore:`, `ci:`, etc., with optional scope e.g. `fix(umount): ...`. Recent history mixes English and Chinese commit bodies — both are fine.
- **Logging**: use the `scoped_log!` macro (`utils/mod.rs`), not bare `log::info!`. Format is `scoped_log!(info, "scope:substage", "key={}, key2={}", a, b)` → emits `[scope:substage] key=..., key2=...`. Prefer structured `key=value` messages, matching existing call sites.
- **Edition 2024**, async-free, `anyhow::Result` for error propagation.

## Gotchas / don't-break list

- **Platform cfg gating**: most of `sys/`, `mount/`, parts of `core/` are `#[cfg(any(target_os = "linux", target_os = "android"))]`. When adding code that calls mount/syscalls, gate it the same way and provide a no-op/stub for other targets so the workspace still type-checks broadly.
- **`build.rs`** generates `module/module.prop` from `Cargo.toml`. Editing `Cargo.toml` `[package]` fields affects generated output.
- **Late-load is supported**: `KSU_LATE_LOAD=1` (KernelSU jailbreak / locked-BL mode) no longer aborts install or startup. KernelSU runs `module/emulated-soft-reboot.sh` during its emulated soft reboot; the script calls `hybrid-mount emulated-soft-reboot`, which detaches the previous run's mounts (source==`config.mountsource` tmpfs/overlay trees, `/mnt/hm_*` storage, and Magic Mount binds sourced from the module dir on managed partitions) before `metamount.sh` mounts again. The same detach also runs at startup when `KSU_LATE_LOAD=1`, so re-mounts never stack.
- **Module layout is never "normalized" at install time.** Keep module files exactly where the user placed them under `/data/adb/modules/<id>/system/*`. Do not move or copy `system/product/*` to top-level `product/*`; users rely on editing files under `system/*` for Magic Mount.
- **Hot install is intentionally unsupported.** `metainstall.sh` must not contain a hot-install implementation; module updates require the normal install + reboot flow.
- **Hardcoded paths** in `defs.rs` (`/data/adb/hybrid-mount/...`, `/data/adb/modules`) are referenced by the shell scripts in `module/` too. If you change one, grep both Rust and `module/*.sh`.
- **`IGNORE_UMOUNT_PARTITIONS` / `MANAGED_PARTITIONS`** in `defs.rs` encode real device-compat decisions (e.g. skipping `*/lib*` to avoid SIGSEGV on pairip-protected libs — see git log). Don't trim them casually.
- **Nano flavor reads mode markers, not config rules**: in nano builds, per-module `overlay`/`magic` marker files (written at install time) select the mount mode, and `default_mode` is forced to `magic`. Logic that assumes the daemon/config is present must stay behind `control-plane`.
- **Legacy Kasumi configs remain loadable.** `default_mode = "kasumi"` and rule path mode `"kasumi"` deserialize as magic mount, and an unknown `[kasumi]` table is ignored. Do not remove that compatibility, and do not reintroduce Kasumi backend code.
- **WebUI ↔ daemon contract**: the daemon speaks JSON over a Unix socket (CLI/automation) and HTTP/SSE over TCP (WebUI). If you change a daemon command or API payload, update both the Rust side (`core/daemon/`, `core/api/`) and the TS side (`webui/src/lib/api/`).
