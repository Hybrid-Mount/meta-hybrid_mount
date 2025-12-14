<img src="icon.svg" align="right" width="120" />

# Meta-Hybrid Mount

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)

**Meta-Hybrid Mount** is a next-generation hybrid mount metamodule designed for KernelSU. Written in native Rust, it intelligently combines **OverlayFS** and **Magic Mount** technologies to provide a more efficient, stable, and stealthy module management experience compared to traditional mounting solutions.

This project includes a modern WebUI management interface built with Svelte, allowing users to monitor status, manage module modes, and view logs in real-time.

**[ 🇨🇳 中文 (Chinese) ](README_ZH.md)**

---

## ✨ Core Features

### 🚀 True Hybrid Engine
* **Smart Strategy**: Prioritizes **OverlayFS** to achieve optimal I/O performance and filesystem merging capabilities.
* **Automatic Fallback**: Automatically and seamlessly falls back to the **Magic Mount** mechanism when OverlayFS mounting fails, the target is unsupported, or when forcibly specified by the user.
* **Rust Native**: The core daemon is written in Rust, utilizing `rustix` for direct system calls, ensuring safety and high efficiency.

### 🛡️ Diagnostics & Safety
* **Conflict Monitor**: Detects and reports file path conflicts between different modules, helping you understand which module overrides which file.
* **System Health**: Built-in diagnostics tool to identify dead symlinks, invalid mount points, and potential bootloop risks before they happen.
* **Paw Pad (Stealth)**: Optional feature to remove `sysfs` traces, making the mount environment harder to detect.

### 🔄 Smart Sync
* **Fast Boot**: Abandons the inefficient pattern of full copying on every boot. The daemon compares `module.prop` checksums and only synchronizes new or modified modules.
* **Dynamic TempDir**: Automatically identifies and utilizes existing empty system directories (e.g., `/debug_ramdisk`) as temporary mount points to minimize traces on `/data`.

## 🖥️ WebUI

The built-in WebUI allows you to:
* **Dashboard**: View storage usage, kernel info, and mount mode statistics.
* **Modules**: Manage per-module mount strategies (Overlay/Magic/HymoFS) and check for file conflicts.
* **Config**: visually edit `config.toml` parameters.
* **Logs**: Real-time daemon log viewer.

---

## 🔨 Build Guide

This project uses Rust's `xtask` pattern for building, integrating the WebUI build process.

### Requirements
* **Rust**: Nightly toolchain (Recommended to use `rustup`)
* **Android NDK**: Version r27+
* **Node.js**: v20+ (For building WebUI)
* **Java**: JDK 17 (For environment configuration)

### Build Commands

1.  **Clone Repository**
    ```bash
    git clone --recursive [https://github.com/YuzakiKokuban/meta-hybrid_mount.git](https://github.com/YuzakiKokuban/meta-hybrid_mount.git)
    cd meta-hybrid_mount
    ```

2.  **Execute Build**
    Use `xtask` to automatically handle WebUI compilation, Rust cross-compilation, and Zip packaging:
    ```bash
    # Build Release version (Includes WebUI and binaries for all architectures)
    cargo run -p xtask -- build --release
    ```

    The build artifacts will be located in the `output/` directory.

3.  **Build Binaries Only (Skip WebUI)**
    If you only modified Rust code, you can skip the WebUI build to save time:
    ```bash
    cargo run -p xtask -- build --release --skip-webui
    ```

### Supported Architectures
The build script compiles the following architectures by default:
* `aarch64-linux-android` (arm64)
* `x86_64-linux-android` (x64)
* `riscv64-linux-android` (riscv64)

---

## 🤝 Contributions & Credits

* Thanks to all contributors in the open-source community.
* Our sister project [Hymo](https://github.com/Anatdx/hymo)
* This project is licensed under the GPL-3.0 License.