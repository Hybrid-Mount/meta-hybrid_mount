# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/master/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)

**Hybrid Mount** is a mount logic metamodule implementation for KernelSU and APatch. It manages module file integration into the Android system using a combination of **OverlayFS** and **bind mounts** (Magic Mount).

The project includes a WebUI dashboard for module management and configuration.

**[🇨🇳 中文 (Chinese)](README_ZH.md)**

---

## Technical Overview

### Mounting Strategies

The core binary (`hybrid-mount`) determines the mounting method for each module directory based on configuration and system compatibility:

1. **OverlayFS**: Uses the kernel's OverlayFS to merge module directories with system partitions. This is the default strategy for supported filesystems.
2. **Magic Mount**: Uses recursive bind mounts to mirror modified file structures. This serves as a fallback strategy when OverlayFS is unavailable or fails.

### Functionality

* **Conflict Detection**: Scans module file paths to identify collisions where multiple modules modify the same file.
* **Module Isolation**: Supports mounting modules in isolated namespaces.
* **Configurable Strategies**: Users can force specific partitions or modules to use OverlayFS or Magic Mount via `config.toml`.
* **Recovery Protocol**: Includes a mechanism to restore default configurations in case of boot failures caused by invalid settings.

---

## Configuration

Configuration is stored at `/data/adb/hybrid-mount/config.toml`.

| Parameter | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `moduledir` | string | `/data/adb/modules/` | Path to the module source directory. |
| `mountsource` | string | Auto-detect | Mount source label (e.g., `KSU`, `APatch`). |
| `partitions` | list | `[]` | List of partitions to explicitly manage. |
| `overlay_mode` | string | `tmpfs` | Backend for loop devices (`tmpfs`, `ext4`, `erofs`). |
| `disable_umount` | bool | `false` | If true, skips unmounting the original source (debug usage). |

---

## WebUI

The project provides a web-based interface built with **SolidJS**.

* **Status**: View current storage usage and kernel version.
* **Management**: Toggle mount modes per module.

---

## Build Instructions

The project uses `xtask` for build automation.

### Prerequisites

* **Rust**: Nightly toolchain.
* **Android NDK**: r27 or newer.
* **Node.js**: v20+ (Required for WebUI compilation).

### Compilation

1. **Full Build (Binary + WebUI)**:

    ```bash
    cargo run -p xtask -- build --release
    ```

    Output will be generated in the `output/` directory.

2. **Binary Only**:

    ```bash
    cargo run -p xtask -- build --release --skip-webui
    ```

---

## License

This project is licensed under the [GPL-3.0 License](LICENSE).
