<img src="icon.svg" align="right" width="120" />

# Meta-Hybrid Mount

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)

**Meta-Hybrid Mount** 是一个专为 KernelSU 设计的下一代混合挂载（Hybrid Mount）元模块。它采用原生 Rust 编写，通过智能结合 OverlayFS 和 Magic Mount 技术，旨在提供比传统挂载方案更高效、更稳定且更具隐蔽性的模块管理体验。

本项目包含一个基于 Svelte 的现代化 WebUI 管理界面，方便用户实时监控状态、管理模块模式及查看日志。

**[ 🇺🇸 English ](README.md)**

---

## ✨ 核心特性

### 🚀 混合挂载引擎 (True Hybrid Engine)
* **智能策略**：优先使用 **OverlayFS** 以获得最佳的 I/O 性能和文件系统合并能力。
* **自动回退**：当 OverlayFS 挂载失败、目标不支持或用户强制指定时，自动无缝回退到 **Magic Mount** 机制，确保最大兼容性。
* **Rust 原生**：核心守护进程使用 Rust 编写，利用 `rustix` 进行直接系统调用，安全且高效。

### 🛡️ 诊断与安全
* **冲突检测 (Conflict Monitor)**：自动分析不同模块之间的文件路径冲突，明确显示哪个模块覆盖了哪个文件。
* **系统健康 (System Health)**：内置诊断工具，可识别死链 (Dead Symlinks)、无效挂载点及潜在的 Bootloop 风险。
* **肉垫模式 (Paw Pad)**：可选功能，移除 `sysfs` 中的挂载痕迹，提升隐蔽性。

### 🔄 智能机制
* **极速启动**：摒弃了每次开机全量复制的低效模式。守护进程会对比 `module.prop`，仅同步新增或发生变化的模块。
* **动态临时目录**：自动识别并利用系统现有的空目录（如 `/debug_ramdisk`）作为临时挂载点，减少在 `/data` 分区的读写痕迹。
* **Umount 控制**：支持“禁用 Umount”及“Umount 共存”模式，完美适配 ZygiskSU 等复杂环境。

## 🖥️ WebUI 管理

访问 WebUI（通常地址为 `ksc://meta-hybrid` 或通过管理器打开），你可以：

* **状态 (Status)**：
    * 查看存储占用及 HymoFS 协议版本。
    * 查看内核版本、SELinux 状态及**系统健康诊断报告**。
* **配置 (Config)**：
    * 可视化编辑 `config.toml`。
    * 开关 **Umount** 相关选项及肉垫模式。
* **模块 (Modules)**：
    * **冲突检测**：一键扫描并展示所有模块间的文件冲突。
    * **模式切换**：针对特定模块，强制指定使用 "OverlayFS"、"Magic Mount" 或 "HymoFS" 模式。
* **日志 (Logs)**：
    * 实时查看守护进程运行日志。

---

## 🔨 构建指南

本项目使用 Rust 的 `xtask` 模式进行构建，并集成了 WebUI 的构建流程。

### 环境要求
* **Rust**: Nightly 工具链 (建议使用 `rustup`)
* **Android NDK**: 版本 r27+
* **Node.js**: v20+ (用于构建 WebUI)
* **Java**: JDK 17 (用于环境配置)

### 构建命令

1.  **克隆仓库**
    ```bash
    git clone --recursive [https://github.com/YuzakiKokuban/meta-hybrid_mount.git](https://github.com/YuzakiKokuban/meta-hybrid_mount.git)
    cd meta-hybrid_mount
    ```

2.  **执行构建**
    使用 `xtask` 自动处理 WebUI 编译、Rust 交叉编译及 Zip 打包：
    ```bash
    # 构建 Release 版本 (包含 WebUI 和所有架构的二进制文件)
    cargo run -p xtask -- build --release
    ```

    构建产物位于 `output/` 目录。

3.  **仅构建二进制 (跳过 WebUI)**
    如果仅修改了 Rust 代码，可以跳过 WebUI 构建以节省时间：
    ```bash
    cargo run -p xtask -- build --release --skip-webui
    ```

---

## 🤝 致谢与协议

* 感谢开源社区的所有贡献者。
* 姊妹项目 [Hymo](https://github.com/Anatdx/hymo)
* 本项目采用 GPL-3.0 协议开源。