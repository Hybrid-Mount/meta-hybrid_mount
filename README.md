# Meta-Hybrid Mount

<img src="https://raw.githubusercontent.com/YuzakiKokuban/meta-hybrid_mount/master/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
[![Telegram](https://img.shields.io/badge/Telegram-@hybridmountchat-2CA5E0?style=flat-square&logo=telegram)](https://t.me/hybridmountchat)

**Meta-Hybrid Mount** is a next-generation hybrid mount metamodule designed for KernelSU and APatch. Written in native Rust, it orchestrates multiple mounting strategies— **OverlayFS** and **Magic Mount**—to provide the ultimate module management experience with superior performance, stability, and stealth.

This project features a modern WebUI built with Svelte, offering real-time status monitoring, granular module configuration, and log inspection.

**[🇨🇳 中文 (Chinese)](https://github.com/YuzakiKokuban/meta-hybrid_mount/blob/master/README_ZH.md)**

---

## ✨ Core Features

### 🚀 Dual Hybrid Engine

Meta-Hybrid intelligently selects the best mounting strategy for each module:

1. **OverlayFS**: Efficient filesystem merging technology that delivers excellent I/O performance.
2. **Magic Mount**: A reliable fallback mechanism used when other methods are unavailable, ensuring maximum compatibility.

### 🛡️ Diagnostics & Safety

* **Conflict Monitor**: Detects file path conflicts between modules, helping you resolve overrides effectively.
* **System Health**: Built-in diagnostics to identify dead symlinks, invalid mount points, and potential bootloop risks.
* **Smart Sync**: Only synchronizes changed modules by comparing `module.prop` checksums, drastically reducing boot time.

### 🔧 Advanced Control

* **Dynamic TempDir**: Automatically utilizes existing empty system directories (e.g., `/debug_ramdisk`) as temporary mount points to minimize traces on `/data`.
* **Umount Strategies**: Configurable unmount behaviors to support complex environments (e.g., ZygiskSU coexistence).

---

## ⚙️ Configuration

The configuration file is located at `/data/adb/meta-hybrid/config.toml`. You can edit it manually or via the WebUI.

| Key | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `moduledir` | string | `/data/adb/modules/` | Directory where modules are installed. |
| `mountsource` | string | `KSU` | Identify the mount source type. |
| `partitions` | list | `[]` | Specific partitions to mount (empty = auto-detect). |
| `enable_nuke` | bool | `false` | Enable aggressive cleanup mode. |
| `force_ext4` | bool | `false` | Force creation of ext4 images for loop devices. |
| `disable_umount` | bool | `false` | Disable unmounting (for troubleshooting). |
| `allow_umount_coexistence` | bool | `false` | Allow coexistence with other unmount solutions. |
| `dry_run` | bool | `false` | Simulate operations without making changes. |
| `verbose` | bool | `false` | Enable detailed logging. |

---

## 🖥️ WebUI

Access the WebUI (via **KernelSU Manager** or browser) to:

* **Dashboard**: Monitor storage and kernel version.
* **Modules**: Toggle mount modes (Overlay/Magic) per module and view file conflicts.
* **Config**: Visually edit `config.toml` parameters.
* **Logs**: Stream the daemon logs in real-time.

---

## 🔨 Build Guide

This project uses Rust's `xtask` pattern for a unified build process.

### Requirements

* **Rust**: Nightly toolchain (via `rustup`)
* **Android NDK**: Version r27+
* **Node.js**: v20+ (for WebUI)
* **Java**: JDK 17 (for environment)

### Build Commands

1. **Clone Repository**

    ```bash
    git clone --recursive [https://github.com/YuzakiKokuban/meta-hybrid_mount.git](https://github.com/YuzakiKokuban/meta-hybrid_mount.git)
    cd meta-hybrid_mount
    ```

2. **Full Build (Release)**
    Compiles WebUI, Rust binaries (arm64, x64, riscv64), and packages the ZIP:

    ```bash
    cargo run -p xtask -- build --release
    ```

    Artifacts will be in `output/`.

3. **Binary Only**
    Skip WebUI build for faster iteration on Rust code:

    ```bash
    cargo run -p xtask -- build --release --skip-webui
    ```

---

## 🤝 Contributions & Credits

* Thanks to all contributors in the open-source community.
* **License**: This project is licensed under the [GPL-3.0 License](https://github.com/YuzakiKokuban/meta-hybrid_mount/blob/master/LICENSE).
