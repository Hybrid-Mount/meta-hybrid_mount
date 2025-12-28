# Meta-Hybrid Mount

<img src="https://raw.githubusercontent.com/YuzakiKokuban/meta-hybrid_mount/master/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
[![Telegram](https://img.shields.io/badge/Telegram-@hybridmountchat-2CA5E0?style=flat-square&logo=telegram)](https://t.me/hybridmountchat)

**Meta-Hybrid Mount** 是专为 KernelSU 和 APatch 设计的下一代混合挂载元模块。它采用原生 Rust 编写，通过智能调度 **OverlayFS** 和 **Magic Mount** 两种挂载策略，为您提供性能卓越、稳定且高度隐蔽的模块管理体验。

本项目包含一个基于 Svelte 构建的现代化 WebUI，支持实时状态监控、精细化模块配置以及日志查看。

**[🇺🇸 English](https://github.com/YuzakiKokuban/meta-hybrid_mount/blob/master/README.md)**

---

## ✨ 核心特性

### 🚀 双重混合引擎 (Dual Engine)

Meta-Hybrid 能够为每个模块智能选择最佳挂载方案：

1. **OverlayFS**：高效的文件系统合并技术，提供最佳的 I/O 读写性能。
2. **Magic Mount**：经典的挂载机制，作为高兼容性的回退方案，确保在任何环境下均可工作。

### 🛡️ 智能诊断与安全

* **冲突监测**：自动检测不同模块间的文件路径冲突，明确展示覆盖关系。
* **系统健康**：内置诊断工具，识别死链 (Dead Symlinks)、无效挂载点及潜在的 Bootloop 风险。
* **极速同步**：守护进程通过对比 `module.prop` 校验和，仅同步变更的模块，大幅缩短开机耗时。

### 🔧 高级控制

* **动态临时目录**：自动复用系统现有的空目录（如 `/debug_ramdisk`）作为挂载点，减少 `/data` 分区痕迹。
* **卸载控制**：支持禁用卸载或与 ZygiskSU 等共存的复杂挂载场景。

---

## ⚙️ 配置文件

配置文件位于 `/data/adb/meta-hybrid/config.toml`，支持手动编辑或通过 WebUI 修改。

| 键名 (Key) | 类型 | 默认值 | 说明 |
| :--- | :--- | :--- | :--- |
| `moduledir` | string | `/data/adb/modules/` | 模块安装目录。 |
| `mountsource` | string | `KSU` | 挂载源类型标识。 |
| `partitions` | list | `[]` | 指定挂载的分区（留空则自动检测）。 |
| `enable_nuke` | bool | `false` | 启用强力清理模式 (Nuke)。 |
| `force_ext4` | bool | `false` | 强制为 Loop 设备使用 ext4 格式。 |
| `disable_umount` | bool | `false` | 禁用卸载操作（用于排错）。 |
| `allow_umount_coexistence`| bool | `false` | 允许与其他卸载方案共存。 |
| `dry_run` | bool | `false` | 空跑模式（仅模拟，不执行更改）。 |
| `verbose` | bool | `false` | 启用详细日志输出。 |

---

## 🖥️ WebUI 管理

通过 KernelSU 管理器或浏览器访问 WebUI：

* **仪表盘 (Dashboard)**：查看存储占用及内核信息。
* **模块 (Modules)**：为每个模块单独切换模式 (Overlay/Magic)，查看文件冲突。
* **配置 (Config)**：可视化编辑 `config.toml` 参数。
* **日志 (Logs)**：实时流式查看守护进程日志。

---

## 🔨 构建指南

本项目使用 Rust 的 `xtask` 模式进行统一构建。

### 环境要求

* **Rust**: Nightly 工具链 (推荐使用 `rustup`)
* **Android NDK**: 版本 r27+
* **Node.js**: v20+ (用于构建 WebUI)
* **Java**: JDK 17 (用于环境配置)

### 构建命令

1. **克隆仓库**

    ```bash
    git clone --recursive [https://github.com/YuzakiKokuban/meta-hybrid_mount.git](https://github.com/YuzakiKokuban/meta-hybrid_mount.git)
    cd meta-hybrid_mount
    ```

2. **完整构建 (Release)**
    编译 WebUI、Rust 二进制文件 (arm64, x64, riscv64) 并打包 ZIP：

    ```bash
    cargo run -p xtask -- build --release
    ```

    构建产物将位于 `output/` 目录。

3. **仅构建二进制**
    跳过 WebUI 构建，加速 Rust 代码开发迭代：

    ```bash
    cargo run -p xtask -- build --release --skip-webui
    ```

---

## 🤝 致谢与协议

* 感谢开源社区的所有贡献者。
* **开源协议**: 本项目遵循 [GPL-3.0 协议](https://github.com/YuzakiKokuban/meta-hybrid_mount/blob/master/LICENSE)
