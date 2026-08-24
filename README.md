# Hybrid Mount

Hybrid Mount 是面向 KernelSU 与 APatch 的混合挂载元模块。它会在启动阶段扫描其他模块，按全局、模块和路径规则，为每一项选择 OverlayFS、Magic Mount 或忽略，并且始终把模块源目录当作只读输入。

## 功能

- OverlayFS 与 Magic Mount 可按模块、按路径混用。
- 路径规则优先于模块默认值，模块默认值优先于全局默认值。
- OverlayFS 支持 tmpfs 与 ext4 两种存储模式。
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
default_mode = "overlay" # overlay | magic | ignore

[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

规则路径相对模块根目录书写。同一路径不能同时进入两个挂载后端，冲突会在启动规划阶段直接报错。配置修改在重启后生效。

## 反馈

安装和反馈问题前请阅读 [使用须知](USAGE_NOTICE.md)。反馈时请附上 KernelSU/APatch bugreport、模块版本与可复现步骤，可通过 [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) 或 [Telegram 群组](https://t.me/hybridmountchat) 联系我们。

## 语言 / Languages

- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)

## 许可证

- 核心（Rust、module 脚本）：GPL-3.0-only（见 [LICENSE](LICENSE)）。
- WebUI：Apache-2.0（见 [webui/LICENSE](webui/LICENSE)）。
