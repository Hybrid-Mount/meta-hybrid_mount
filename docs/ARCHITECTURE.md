# Hybrid Mount 接口参考

## CLI 契约

设备上的可执行文件通常是 `/data/adb/modules/hybrid_mount/hybrid-mount`。无参数时直接运行启动挂载流水线；命令失败会写入 stderr 并以非零状态退出。

| 命令 | 参数 | 输出与行为 |
| --- | --- | --- |
| *(无参数)* | 无 | 执行完整启动挂载流水线。 |
| `boot` | `[metamount\|post-mount]` | 带启动锁与本轮去重的流水线入口；阶段选择由根管理器类型决定。 |
| `runtime` | 子命令 | 挂载与规则事务管理，见 [Runtime](RUNTIME.md)。 |
| `show-config` | 无 | 输出当前有效配置 JSON。 |
| `save-config` | `--payload <hex>` | 合并十六进制 UTF-8 JSON patch，成功输出 `{ "ok": true }`。 |
| `gen-config` | 无 | 写入默认配置，成功输出 `{ "ok": true }`。 |
| `modules` | 无 | 输出模块与规则快照 JSON；缺失或损坏时重建。 |
| `status` | 无 | 输出启动状态 JSON；缺失时输出默认状态，只读且不触发加载。 |
| `install-state` | 无 | 输出安装与内核兼容状态 JSON。 |
| `clear-mount-errors` | 无 | 删除模块目录中的 `mount_error` 文件并刷新状态，输出 `{ "ok": true, "removed": <数量> }`。 |
| `vfs-doctor` | 无 | 只读输出 VFS provider 诊断 JSON，不会 `insmod` 或卸载模块。 |
| `vfs` | `help` / `rule` / `uid` / `clear` / `version` / `doctor` / `load` | VFS 运行态控制；默认文本，支持 `--json`、批量参数及 NoMount 风格别名。清空需要 `--yes`，仅显式 `load` 加载模块。见 [VFS CLI](VFS_CLI.md)。 |
| `lkm-load` | `<module.ko> [parameters...]` | Linux/Android arm64 上以内置加载器插入指定 LKM；仅支持 aarch64。 |
| `emulated-soft-reboot` | 无 | 兼容别名，执行 `runtime prepare-reboot`，仅清理身份验证通过的本轮资源。 |
| `version` | 无 | 输出版本 JSON。 |

示例：

```sh
BIN=/data/adb/modules/hybrid_mount/hybrid-mount
$BIN version
$BIN status
$BIN vfs-doctor
```

`save-config` 的 payload 是 JSON patch 的十六进制 UTF-8 编码，不是 TOML；WebUI 使用同一接口。

WebUI 使用同一 CLI；状态快照由启动或成功的热操作更新，详见 [Runtime](RUNTIME.md)。

`status` 中的 `active_mounts` 是 OverlayFS、Magic Mount 与 VFS 成功目标合并、排序、去重后的兼容字段；`overlay_active_mounts`、`magic_active_mounts` 与 `vfs_active_mounts` 保留分后端明细。Magic Mount 只把成功的文件 bind 目标和目录 mount-move 目标计入活动挂载点，符号链接创建仍只进入操作统计，不伪装成挂载点。OverlayFS 与 Magic Mount 的目标必须经 mountinfo 确认；VFS 注入点不是内核挂载，改由 `apply_vfs_phase` 的 Provider 读回确认，读回失败的批次会被整体丢弃，因此不会进入 `active_mounts`。

`status` 另外暴露 VFS 字段：`vfs_modules` 列出本次启动使用 VFS 的模块，`vfs_active_mounts` 记录注入成功的目标路径，`vfs_provider` 为本次启动唯一绑定的内核 Provider（只有 `hm`，即 HM 自有的 `hybridmount` 模块）。这些字段与配置一并由启动流水线与状态层写入启动快照。

`storage_mode` 记录本次启动实际创建的 overlay staging 后端，取值为 `tmpfs`、`ext4` 或哨兵值 `none`。只使用 VFS 或 Magic Mount 的启动不会创建 staging，此时写入 `none`：动态模块描述与 WebUI 都据此显示实际运行的挂载后端（例如 VFS），不会假报 Tmpfs 或 Ext4。
