# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/master/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)

Hybrid Mount 是面向 **KernelSU** 与 **APatch** 的挂载编排元模块。  
它使用混合策略把模块文件注入 Android 分区：

- 内核/文件系统条件允许时优先使用 **OverlayFS**。
- 不满足条件或按规则指定时回退到 **Magic Mount（bind mount）**。

整体目标是：启动行为可预测、冲突可观测、策略可配置。

**[🇺🇸 English](README.md)**

---

## 目录

- [设计目标](#设计目标)
- [架构说明](#架构说明)
- [仓库结构](#仓库结构)
- [配置说明](#配置说明)
- [CLI 命令](#cli-命令)
- [构建方式](#构建方式)
- [运维建议](#运维建议)
- [开源协议](#开源协议)

---

## 设计目标

1. **兼容优先**：适配不同 Android 内核环境。
2. **可确定性**：通过显式规划减少“偶现挂载异常”。
3. **运行安全性**：配置和恢复流程尽可能保守。
4. **自动化友好**：CLI 输出可直接给 WebUI/脚本消费。

## 架构说明

`hybrid-mount` 启动后主要流程如下：

1. 加载配置（文件 + CLI 覆盖）。
2. 扫描模块目录并构建清单。
3. 生成执行计划（overlay/magic/ignore）。
4. 执行挂载并记录运行状态。
5. 按需输出冲突与诊断报告。

关键模块：

- `src/conf`：配置模型、加载器、CLI 处理。
- `src/core/inventory`：模块扫描与数据建模。
- `src/core/ops`：计划生成、执行与同步。
- `src/mount`：overlayfs 与 magic mount 后端。
- `src/sys`：底层文件系统与挂载接口。

## 仓库结构

```text
.
├─ src/                 # 守护进程与运行时逻辑
├─ module/              # 模块脚本与打包资源
├─ xtask/               # 构建/发布自动化入口
├─ tools/notify/        # 可选辅助工具
├─ Cargo.toml           # workspace 与主 crate 配置
└─ README*.md           # 中英文文档
```

## 配置说明

默认路径：`/data/adb/hybrid-mount/config.toml`。

### 顶层字段

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | 模块目录。 |
| `mountsource` | string | 自动检测 | 运行来源标识（如 `KSU`、`APatch`）。 |
| `partitions` | list\|csv string | `[]` | 额外受管分区。 |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Overlay 上层存储模式。 |
| `disable_umount` | bool | `false` | 跳过 umount（仅调试建议使用）。 |
| `allow_umount_coexistence` | bool | `false` | 允许与既有 umount 行为共存。 |
| `enable_overlay_fallback` | bool | `false` | 当 overlayfs 不可用时，允许将 overlay 计划模块回退到 Magic Mount。 |
| `default_mode` | `overlay` \| `magic` | `overlay` | 全局默认策略。 |
| `rules` | map | `{}` | 按模块 + 路径细粒度策略。 |

### 示例

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

## CLI 命令

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

全局参数：

- `-c, --config <PATH>` 指定配置文件路径
- `-m, --moduledir <PATH>` 覆盖模块目录
- `-s, --mountsource <SOURCE>` 覆盖来源标识
- `-p, --partitions <CSV>` 覆盖分区列表

子命令：

- `gen-config` 生成配置文件
- `show-config` 输出当前生效配置（JSON）
- `save-config --payload <HEX_JSON>` 从 WebUI 负载保存配置
- `save-module-rules --module <ID> --payload <HEX_JSON>` 更新单模块规则
- `modules` 输出模块清单

## 构建方式

环境要求：

- 使用 `rust-toolchain.toml` 指定的 Rust 工具链
- Android NDK（建议 r27+）
- Node.js 20+（仅构建 WebUI 时需要）

命令示例：

```bash
# 完整构建
cargo run -p xtask -- build --release

# 仅构建运行时（二进制）
cargo run -p xtask -- build --release --skip-webui
```

产物输出到 `output/`。

## 运维建议

- 如果配置导致启动异常，先 `gen-config` 生成最小配置，再逐步恢复规则。
- 缩小体积建议优先从依赖特性裁剪与 release profile 入手，再考虑重构。

## 开源协议

本项目采用 [GPL-3.0](LICENSE)。
