# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount 是面向 KernelSU 与 APatch 的混合挂载元模块。它会在启动阶段扫描其他模块，按全局、模块和路径规则，为每一项选择 OverlayFS、Magic Mount、VFS 或忽略，并且始终把模块源目录当作只读输入。

## 功能

- OverlayFS、Magic Mount 与 VFS 可按模块、按路径混用。
- 路径规则优先于模块默认值，模块默认值优先于全局默认值。
- OverlayFS 支持 tmpfs 与 ext4 两种存储模式。
- ext4 staging 在 KernelSU 使用官方 ioctl 隐藏 sysfs 节点；在 APatch 等非 KSU 环境默认使用随附 LKM 兼容后备。
- Magic Mount 支持文件、目录、符号链接、`.replace` 和 whiteout 语义。
- VFS 通过 keyring 把注入规则下发给 HM 自有的 VFS 内核子系统（`hybridmount` 模块）。它是独立实现，不与 NoMount 内核或其 nm CLI 互操作。发布包同时提供源码与每个受支持 Android/GKI 目标的 arm64 预编译模块，内核未内建时由启动流程自动加载；仍不可用时按 `vfs_strict` 降级。VFS 不是真实挂载。
- WebUI 提供 MD3（默认）与 Miuix 两套界面。
- 支持 arm64、armv7 与 x86_64，安装脚本会自动选择对应二进制。

## 安装

从 [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) 下载 ZIP，并在 KernelSU 或 APatch 管理器中安装。首次安装可用音量键选择默认后端；升级时会保留 `/data/adb/hybrid-mount/config.toml`。

## 配置

默认配置：

```toml
moduledir = "/data/adb/modules"
overlay_mode = "ext4" # ext4 | tmpfs
disable_umount = false
default_mode = "overlay" # overlay | magic | vfs

[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

规则路径相对模块根目录书写。模块级和路径级规则仍可使用 `ignore`；全局默认后端接受 `overlay`、`magic` 或 `vfs`。`vfs_strict = true` 时 VFS 不可用即启动失败，`vfs_isolate_uids` 列出应看到原生文件系统的 UID。同一文件路径不能同时进入多个后端；Overlay 与 Magic 两个真实挂载后端可以共享普通目录作为结构节点，VFS 是第三条注入路径、不是真实挂载。文件、类型或 `.replace` 冲突会在启动规划阶段直接报错。配置修改在重启后生效。

这套分流不改变项目现有的 `CONFIG_TMPFS_XATTR` 能力判断。KernelSU 安装时会删除模块中的整个 `lkm/` 目录，运行时只使用官方 `NukeExt4Sysfs` ioctl；APatch 等非 KSU 安装保留 LKM，并在 ext4 staging 挂载后默认尝试。随附 `.ko` 仅支持 aarch64；自动选择要求内核线和 Android/GKI 标签精确匹配，未知组合直接拒绝，但预编译 LKM 仍必须在对应真机验证 ABI。若设备在 `insmod` 期间崩溃，持久熔断标记会阻止下次启动再次加载 LKM，同时保留 Hybrid Mount 的其余功能。支持矩阵、校验值、来源与许可见 [`module/lkm/README.md`](../module/lkm/README.md)。

## VFS 后端

VFS 是 Hybrid Mount 自有的内核侧注入路径，由 `hybridmount` 模块经 keyring 驱动。它是独立实现，不与 NoMount 互操作。

**如何识别 Provider。** 启动决策只看对内核 key type `hybridmount` 的一次只读探测：只要它返回受支持的版本，Provider 即可用。`vfs-doctor` 另外负责判断它以何种方式存在——出现在 `/proc/modules` 中，说明由可加载模块注册；有 `/sys/module/hybridmount` 目录但没有上述条目，说明已编译进内核镜像；两者都没有，说明本机没有 Provider。探测是只读的，因此 `status` 与 `vfs-doctor` 都不会触发 `insmod`。

**启动逻辑。** 如果 key type 返回受支持的版本，Provider 即被绑定，不加载任何东西。每次启动都会探测 VFS 能力；即使没有规则选择 VFS，只要探测无响应，流水线会挑选与内核线及 Android/GKI 标签精确匹配的随附模块，加载后重新探测；仍不可用时，所有 `vfs` 规则降级为 `ignore`，确有规则选择 VFS 且 `vfs_strict = true` 时则启动失败。加载发生在挂载计划构建之前，因为规划阶段会在 Provider 无响应时把 `vfs` 规则改写为 `ignore`，执行器随后就会提前返回。熔断标记在 `insmod` 前写入，尝试返回时清除，因此只有内核崩溃才会把它留下；下次启动将拒绝自动重试，直到手动删除该标记。

**把 VFS 集成进内核。** 发布包为每个受支持的 Android/GKI 目标都提供 aarch64 预编译模块并自动加载，因此这些内核无需任何集成步骤。当你希望避免 `insmod`，或你的内核线没有对应预编译模块时，可以把它内建进内核。在内核源码树根目录执行：

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash
```

`--cleanup`:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash -s -- --cleanup
```

这会把源码复制到 `fs/hybridmount/`，并加入 `fs/Makefile` 与 `fs/Kconfig`；启用 `CONFIG_HYBRIDMOUNT=y` 表示内建，`=m` 表示编译为模块。`bash -s -- --cleanup` 会撤销全部改动。已集成 NoMount 的内核树会被拒绝：两种实现都会劫持 inode 操作，而由于它们注册的 key type 不同，内核不会阻止二者并存。

Provider 不可用时，WebUI 隐藏 VFS 选项和统计，管理器描述也不显示 VFS 计数；已有规则保持不变。内建 Provider 只要探测正常，即使没有 `/proc/modules` 条目也仍然支持。

**诊断。** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor` 会报告存在状态、key type 返回的版本、受支持的版本，以及 Provider 不可用时的原因。

## 反馈

安装和反馈问题前请阅读 [使用须知](../USAGE_NOTICE.md)。反馈时请附上 KernelSU/APatch bugreport、模块版本与可复现步骤，可通过 [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) 或 [Telegram 群组](https://t.me/hybridmountchat) 联系我们。

## 语言 / Languages

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ES.md)
- [Français](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_FR.md)
- [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ID.md)
- [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_IT.md)
- [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_JA.md)
- [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_RU.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)
- [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_UK.md)
- [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_VI.md)
- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH.md)
- [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH_TW.md)

## 鸣谢

- 感谢 [Anatdx](https://github.com/Anatdx)
- 感谢 [Tools-cx-app](https://github.com/Tools-cx-app)
- 感谢 [KernelSU](https://github.com/tiann/KernelSU)
- 感谢 [5ec1cff 的 MKSU](https://github.com/5ec1cff/KernelSU)
- 感谢 [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- 感谢 [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- 感谢 [NoMount](https://github.com/maxsteeel/nomount)

## 许可证

- 核心（Rust、module 脚本）：GPL-3.0-only（见 [LICENSE](../LICENSE)）。
- WebUI：Apache-2.0（见 [webui/LICENSE](../webui/LICENSE)）。
- 可选 ext4 sysfs LKM（源码与预编译 `.ko`）：GPL-2.0-only，源自 [Mountify](https://github.com/backslashxx/mountify)；见 [module/lkm/README.md](../module/lkm/README.md) 与 [module/lkm/src/LICENSE](../module/lkm/src/LICENSE)。
- VFS 内核子系统（`hybridmount` 模块）：GPL-2.0-only，fork 自 [NoMount](https://github.com/maxsteeel/nomount)；归属与许可证细节见 [module/vfs/README.md](../module/vfs/README.md)、[module/vfs/src/LICENSE](../module/vfs/src/LICENSE) 与 [THIRD_PARTY.md](../THIRD_PARTY.md)。
