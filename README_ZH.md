# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-Apache--2.0-blue?style=flat-square)

Hybrid Mount 是面向 **KernelSU** 与 **APatch** 的挂载编排元模块。  
它现在支持三种挂载方式，把模块文件注入 Android 分区：

- **OverlayFS**：兼容优先的分层挂载。
- **Magic Mount（bind mount）**：直接路径绑定或回退方案。
- **Kasumi**：用于显式 `kasumi` 路由，以及依赖 Kasumi runtime 的 hide/spoof 能力。

整体目标是：启动行为可预测、冲突可观测、策略可配置。

**[🇺🇸 English](README.md)**

---

## 目录

- [设计目标](#设计目标)
- [挂载方式](#挂载方式)
- [架构说明](#架构说明)
- [仓库结构](#仓库结构)
- [配置说明](#配置说明)
- [Kasumi](#kasumi)
- [策略行为矩阵](#策略行为矩阵)
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

## 挂载方式

Hybrid Mount 当前支持三种后端策略：

- `overlay`：适合可安全合并的模块路径，走 OverlayFS。
- `magic`：适合直接替换或回退场景，走 Magic Mount bind mount。
- `kasumi`：模块或路径显式指定为 `kasumi` 时，交给 Kasumi mirror/runtime 处理。

## 架构说明

`hybrid-mount` 启动后主要流程如下：

1. 加载配置（文件 + CLI 覆盖）。
2. 扫描模块目录并构建清单。
3. 生成执行计划（overlay/magic/kasumi/ignore）。
4. 执行挂载并记录运行状态。
5. 按需输出冲突与诊断报告。

关键模块：

- `src/conf`：配置模型、加载器、CLI 处理。
- `src/core/inventory`：模块扫描与数据建模。
- `src/core/ops`：计划生成、执行与同步。
- `src/mount`：OverlayFS、Magic Mount 与 Kasumi 后端。
- `src/sys`：底层文件系统与挂载接口。

## 仓库结构

```text
.
├─ src/                 # 守护进程与运行时逻辑
├─ module/              # 模块脚本与打包资源
├─ xtask/               # 构建/发布自动化入口
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
| `enable_overlay_fallback` | bool | `false` | 当 overlayfs 不可用时，允许将 overlay 计划模块回退到 Magic Mount。 |
| `default_mode` | `overlay` \| `magic` \| `kasumi` | `overlay` | 全局默认策略。 |
| `rules` | map | `{}` | 按模块 + 路径细粒度策略。 |

### 示例

```toml
moduledir = "/data/adb/modules"
mountsource = "KSU"
partitions = ["system", "vendor"]
overlay_mode = "ext4"
disable_umount = false
enable_overlay_fallback = false
default_mode = "overlay"

[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/tool" = "overlay"
"vendor/lib64/libfoo.so" = "ignore"
```

## Kasumi

`Kasumi` 是 Hybrid Mount 的第三种挂载后端。它由内核/LKM 提供支持，既可用于显式的 `kasumi` 路由，也承担 Kasumi 专属的运行时 hide/spoof 能力。

它在项目里主要承担两类工作：

- `mode = "kasumi"` 的挂载映射：把模块或路径解析到 Kasumi mirror 树中的内容
- 额外运行时特性：stealth/hide-xattr、mount hide、`/proc/<pid>/maps` 伪装、`statfs` 伪装、UID 隐藏、uname/cmdline 伪装，以及按目标生效的 kstat 伪装规则

### 什么时候会真正启用运行时

`kasumi.enabled = true` 只是允许使用 Kasumi。Hybrid Mount 只有在满足下面任一条件时，才会真正把 Kasumi runtime 打开：

- 生成出来的挂载计划中存在至少一个 Kasumi 模块或路径
- 配置了任一辅助特性：`enable_hidexattr`、`enable_mount_hide`、`enable_maps_spoof`、`enable_statfs_spoof`、`hide_uids`、`cmdline_value`、`uname*`、`maps_rules`、`kstat_rules`，或持久化的 user hide 规则

几个和实际行为强相关的细节：

- `enable_hidexattr` 是兼容模式总开关，实际会一并启用 `stealth`、`mount_hide`、`maps_spoof`、`statfs_spoof`
- `mount_hide.path_pattern` 与 `statfs_spoof.{path,spoof_f_type}` 本身也会让对应特性被判定为启用
- CLI 里执行 disable 时，现在会同步清空这些结构化附属字段，避免“明明关了但因为残留参数又被判定为开启”的情况

### 关键配置项

| 字段 | 作用 |
| --- | --- |
| `kasumi.enabled` | Kasumi 集成总开关。 |
| `kasumi.lkm_autoload` | 启动时是否尝试自动加载 Kasumi LKM。 |
| `kasumi.lkm_dir` / `kasumi.lkm_kmi_override` | LKM 搜索目录与可选 KMI 覆盖。 |
| `kasumi.mirror_path` | Kasumi 规则使用的 mirror 根目录，默认 `/dev/kasumi_mirror`。 |
| `kasumi.enable_kernel_debug` | 打开内核侧 debug 输出。 |
| `kasumi.enable_stealth` | 显式启用 stealth。 |
| `kasumi.enable_hidexattr` | 兼容模式总开关，会联动多项 hide/spoof 能力。 |
| `kasumi.enable_mount_hide` / `kasumi.mount_hide.path_pattern` | 全局或按路径模式启用 mount hide。 |
| `kasumi.enable_maps_spoof` / `kasumi.maps_rules` | 启用 maps spoof，并安装 inode/device 映射规则。 |
| `kasumi.enable_statfs_spoof` / `kasumi.statfs_spoof.*` | 启用通用或按路径生效的 `statfs` 伪装。 |
| `kasumi.hide_uids` | 配置需要隐藏的 UID 集合。 |
| `kasumi.uname.*` | 结构化 uname 伪装配置。 |
| `kasumi.cmdline_value` | 替换内核 cmdline 内容。 |
| `kasumi.kstat_rules` | 按目标应用的 stat 元数据伪装规则。 |

### 示例

```toml
[kasumi]
enabled = true
lkm_autoload = true
mirror_path = "/dev/kasumi_mirror"
enable_mount_hide = true

[rules.my_module]
default_mode = "kasumi"

[rules.my_module.paths]
"system/bin/su" = "kasumi"
```

### 常用命令

```bash
# 查看运行时/LKM 状态
hybrid-mount kasumi status
hybrid-mount kasumi version
hybrid-mount kasumi features
hybrid-mount lkm status

# 配置并实时同步常见特性
hybrid-mount kasumi enable
hybrid-mount kasumi disable
hybrid-mount kasumi mount-hide enable --path-pattern /dev/kasumi_mirror
hybrid-mount kasumi statfs-spoof enable --path /system --f-type 0x794c7630
hybrid-mount kasumi maps add --target-ino 1 --target-dev 2 --spoofed-ino 3 --spoofed-dev 4 --path /dev/kasumi_mirror/system/bin/sh
hybrid-mount kasumi kstat upsert --target-ino 11 --target-path /system/bin/app_process64 --spoofed-ino 22 --spoofed-dev 33
```

运维注意：

- `kasumi kstat clear-config` 只会移除持久化配置；已经下发到内核侧的 kstat 规则，通常仍需要重载 Kasumi LKM 或重建整套 runtime 才会完全清掉。

## 策略行为矩阵

下表用于说明不同策略在不同运行条件下的实际行为：

| 规则结果 | 后端可用性 | `enable_overlay_fallback` | 最终行为 |
| --- | --- | --- | --- |
| `overlay` | OverlayFS 可用 | 任意 | 使用 OverlayFS 挂载。 |
| `overlay` | OverlayFS 不可用 | `false` | 跳过挂载，并在计划/执行结果中标记失败项。 |
| `overlay` | OverlayFS 不可用 | `true` | 回退为 Magic Mount（bind mount）重试。 |
| `magic` | 不适用 | 任意 | 直接使用 Magic Mount。 |
| `kasumi` | Kasumi 可用 | 任意 | 直接使用 Kasumi 挂载。 |
| `kasumi` | Kasumi 不可用或未启用 | 任意 | 跳过该路径/模块的 Kasumi 映射。 |
| `ignore` | 不适用 | 任意 | 不挂载该路径。 |

### 规则优先级

当多个策略可能同时命中时，优先级如下：

1. 路径级覆盖（`rules.<module>.paths["..."]`）
2. 模块级默认（`rules.<module>.default_mode`）
3. 全局默认（`default_mode`）

### 实用示例

- 模块大部分路径走 overlay，仅单个易冲突文件走 magic：
  - 模块默认设为 `overlay`
  - 对该路径配置 `rules.<module>.paths["system/bin/<tool>"] = "magic"`
- 临时屏蔽单个冲突文件，而不禁用整个模块：
  - 配置 `rules.<module>.paths["..."] = "ignore"`
- 内核 OverlayFS 稳定性不足时降低失败概率：
  - 配置 `enable_overlay_fallback = true`。

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

# 本地 arm64 调试包
./scripts/build-local.sh

# 打入预编译的 Kasumi LKM 资产
./scripts/build-local.sh --release --kasumi-lkm-dir /path/to/kasumi-lkm
```

产物输出到 `output/`。

## 运维建议

- 新安装默认依赖自动检测 `mountsource`，只有在 `config.toml` 中显式指定时才会覆盖。
- 如果配置导致启动异常，先 `gen-config` 生成最小配置，再逐步恢复规则。
- 缩小体积建议优先从依赖特性裁剪与 release profile 入手，再考虑重构。

## 开源协议

本仓库采用 [Apache-2.0](LICENSE)。
