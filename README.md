# Hybrid Mount

Hybrid Mount 是面向 KernelSU 与 APatch 的混合挂载元模块。它会在启动阶段扫描其他模块，按全局、模块和路径规则，为每一项选择 OverlayFS、Magic Mount 或忽略，并且始终把模块源目录当作只读输入。

## 功能

- OverlayFS 与 Magic Mount 可按模块、按路径混用。
- 路径规则优先于模块默认值，模块默认值优先于全局默认值。
- OverlayFS 支持 tmpfs 与 ext4 两种存储模式。
- ext4 staging 在 KernelSU 使用官方 ioctl 隐藏 sysfs 节点；在 APatch 等非 KSU 环境默认使用随附 LKM 兼容后备。
- Magic Mount 支持文件、目录、符号链接、`.replace` 和 whiteout 语义。
- WebUI 提供 MD3（默认）与 Miuix 两套界面。
- 支持 arm64、armv7 与 x86_64，安装脚本会自动选择对应二进制。

## 安装

从 [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) 下载 ZIP，并在 KernelSU 或 APatch 管理器中安装。首次安装可用音量键选择默认后端；升级时会保留 `/data/adb/hybrid-mount/config.toml`。

## 配置

默认配置：

```toml
moduledir = "/data/adb/modules"
mountsource = "KSU"
overlay_mode = "ext4" # ext4 | tmpfs
disable_umount = false
default_mode = "overlay" # overlay | magic

[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

规则路径相对模块根目录书写。模块级和路径级规则仍可使用 `ignore`；全局默认后端只接受 `overlay` 或 `magic`。同一文件路径不能同时进入两个挂载后端；普通目录可以作为两个后端共享的结构节点，文件、类型或 `.replace` 冲突会在启动规划阶段直接报错。配置修改在重启后生效。

这套分流不改变项目现有的 `CONFIG_TMPFS_XATTR` 能力判断。KernelSU 安装时会删除模块中的整个 `lkm/` 目录，运行时只使用官方 `NukeExt4Sysfs` ioctl；APatch 等非 KSU 安装保留 LKM，并在 ext4 staging 挂载后默认尝试。随附 `.ko` 仅支持 aarch64；自动选择要求内核线和 Android/GKI 标签精确匹配，未知组合直接拒绝，但预编译 LKM 仍必须在对应真机验证 ABI。若设备在 `insmod` 期间崩溃，持久熔断标记会阻止下次启动再次加载 LKM，同时保留 Hybrid Mount 的其余功能。支持矩阵、校验值、来源与许可见 [`module/lkm/README.md`](module/lkm/README.md)。

## 反馈

安装和反馈问题前请阅读 [使用须知](USAGE_NOTICE.md)。反馈时请附上 KernelSU/APatch bugreport、模块版本与可复现步骤，可通过 [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) 或 [Telegram 群组](https://t.me/hybridmountchat) 联系我们。

## 语言 / Languages

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_EN.md)
- [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ES.md)
- [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ID.md)
- [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_IT.md)
- [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_JA.md)
- [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_RU.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)
- [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_UK.md)
- [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_VI.md)
- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH_TW.md)

## 许可证

- 核心（Rust、module 脚本）：GPL-3.0-only（见 [LICENSE](LICENSE)）。
- WebUI：Apache-2.0（见 [webui/LICENSE](webui/LICENSE)）。
- 可选 ext4 sysfs LKM（源码与预编译 `.ko`）：GPL-2.0-only，源自 [Mountify](https://github.com/backslashxx/mountify)；见 [module/lkm/README.md](module/lkm/README.md) 与 [module/lkm/src/LICENSE](module/lkm/src/LICENSE)。
