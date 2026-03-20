# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/master/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)

**Hybrid Mount** 是 KernelSU 和 APatch 的挂载逻辑元模块实现。它结合 **OverlayFS** 和 **Bind Mounts** (Magic Mount) 将模块文件集成到 Android 系统中。

本项目包含一个基于 **SolidJS** 构建的 WebUI 面板，用于模块管理和配置。

**[🇺🇸 English](README.md)**

---

## 技术概览

### 挂载策略

核心二进制程序 (`hybrid-mount`) 会根据配置和系统兼容性为每个模块目录决定挂载方式：

1. **OverlayFS**：使用内核的 OverlayFS 将模块目录与系统分区合并。这是支持该文件系统的设备上的默认策略。
2. **Magic Mount**：使用递归 Bind Mount 镜像修改后的文件结构。当 OverlayFS 不可用或失败时，此策略作为回退方案运行。

### 功能特性

* **冲突检测**：扫描模块文件路径，识别多个模块修改同一文件时的冲突情况。
* **模块隔离**：支持在隔离的命名空间中挂载模块。
* **策略配置**：用户可通过 `config.toml` 强制特定分区或模块使用 OverlayFS 或 Magic Mount。
* **恢复协议**：包含故障恢复机制，若因配置无效导致启动失败，将自动恢复默认配置。

---

## 配置

配置文件位于 `/data/adb/hybrid-mount/config.toml`。

| 参数 | 类型 | 默认值 | 说明 |
| :--- | :--- | :--- | :--- |
| `moduledir` | string | `/data/adb/modules/` | 模块源目录路径。 |
| `mountsource` | string | 自动检测 | 挂载源标签 (如 `KSU`, `APatch`)。 |
| `partitions` | list | `[]` | 显式管理的分区列表。 |
| `overlay_mode` | string | `ext4` | Loop 设备后端类型 (`tmpfs`, `ext4`, `erofs`)。 |
| `disable_umount` | bool | `false` | 若为 true，则跳过卸载原始源（调试用途）。 |

---

## WebUI

项目提供了一个基于 **SolidJS** 开发的 Web 管理界面。

* **状态**：查看当前存储使用情况和内核版本。
* **管理**：切换模块的挂载模式。

---

## 构建指南

本项目使用 `xtask` 进行自动化构建。

### 环境要求

* **Rust**: Nightly 工具链。
* **Android NDK**: r27 或更新版本。
* **Node.js**: v20+ (编译 WebUI 所需)。

### 编译命令

1. **完整构建 (二进制 + WebUI)**：

    ```bash
    cargo run -p xtask -- build --release
    ```

    构建产物将生成在 `output/` 目录中。

2. **仅构建二进制**：

    ```bash
    cargo run -p xtask -- build --release --skip-webui
    ```

### 致谢

* [5ec1cff/KernelSU](https://github.com/5ec1cff/KernelSU/blob/52f1f575ce2bd0ca46ebf644fd00a838af9f344e/userspace/ksud/src/magic_mount.rs)：原始实现
* [Tools-cx-app/meta-magic_mount](https://github.com/Tools-cx-app/meta-magic_mount-rs)：magic mount 部分
* [KernelSU-Modules-Repo/meta-overlayfs](https://github.com/KernelSU-Modules-Repo/meta-overlayfs) overlayfs 参考
* [bmax121/APatch](https://github.com/bmax121/APatch/blob/8e4b71ada83e06c02cf03b23debe9bf7c9dc1e9e/apd/src/mount.rs) overlayfs 参考

---

## 协议

本项目遵循 [GPL-3.0 协议](LICENSE) 开源。
