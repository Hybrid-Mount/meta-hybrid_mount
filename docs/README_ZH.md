# Hybrid Mount

[AtomGit 仓库](https://atomgit.com/Hybrid-Mount/meta-hybrid_mount) · [GitHub 仓库](https://github.com/Hybrid-Mount/meta-hybrid_mount) · [参与贡献](../CONTRIBUTING.md)

Hybrid Mount 是 KernelSU 和 APatch 的挂载元模块。启动时，它扫描其他模块的文件，按规则使用 OverlayFS、Magic Mount 或内核侧 VFS。模块源目录始终只读。

## 安装

阅读[使用须知](../USAGE_NOTICE.md)，从 [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) 下载 ZIP，在 KernelSU 或 APatch 管理器中安装。首次安装用音量键选择默认后端，升级会保留已有配置。

## 配置

编辑 `/data/adb/hybrid-mount/config.toml`，重启后生效。路径规则优先于模块默认值，模块默认值优先于全局默认值。模块或路径规则可设为 `ignore` 以跳过相应内容。下例让一个模块使用 Magic Mount，其中的 hosts 文件单独使用 OverlayFS。

```toml
[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

存储模式和额外分区选项见[默认配置](../module/config.toml)。VFS 需要兼容的内核后端。后端不可用时，默认跳过 VFS 规则；启用 `vfs_strict = true` 且有规则请求 VFS 时，本次挂载流程会失败。

将 Lua 5.5.1 脚本放入 `/data/adb/hybrid-mount/lua/`，重启后生效。API 1 的 `before()` 和 `after()` 回调可调用 `bind_mount`、`overlay_mount` 和 `vfs_mount`。API 2 在 `load()` 中用 `change_mount(path, mode)` 按模块相对路径为文件或子树选择 `vfs`、`overlay` 或 `magic`。[脚本示例和 API 说明](CUSTOM_MOUNT.md)

- [规则、目录标记与 CLI](ARCHITECTURE.md) · [运行时挂载与恢复](RUNTIME.md)
- [VFS 命令与诊断](VFS_CLI.md) · [VFS 内核集成](../module/vfs/README.md)
- [ext4 兼容模块](../module/lkm/README.md)
- [问题反馈与贡献指南](../CONTRIBUTING.md)

## 其他语言

[English](../README.md) · [Español](README_ES.md) · [Français](README_FR.md) · [Bahasa Indonesia](README_ID.md) · [Italiano](README_IT.md) · [日本語](README_JA.md) · [Русский](README_RU.md) · [Türkçe](README_TR.md) · [Українська](README_UK.md) · [Tiếng Việt](README_VI.md) · [简体中文](README_ZH.md) · [繁體中文](README_ZH_TW.md)

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
