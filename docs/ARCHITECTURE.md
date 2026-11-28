# Hybrid Mount 接口参考

源码目录和模块依赖见[项目架构](PROJECT_LAYOUT.md)。

目录中的普通 `.remove` 文件规范化为目录路径的 whiteout，扫描停止收集其后代；`.remove` 优先于 `.replace`。规划在原逻辑路径上匹配 ignore，再应用实际分区映射与根/子挂载保护。跨模块 whiteout 与内容贡献冲突在执行前报错。Overlay staging 创建 0:0 字符设备，Magic 父镜像省略目标，VFS 发出已有 Whiteout 动作；不删除原目录，不改变协议。

## Lua 扩展与挂载顺序

1. 启动流程探测分区后，`src/framework/extension/` 按文件名加载 `/data/adb/hybrid-mount/lua/` 下的 Lua 5.5.1 脚本，先收集 API 1 的 `before()` 请求。
2. 扫描每个符合条件的模块时，先运行匹配该 ID 的 API 2 `load()`。规划器合并 `change_mount` 与 TOML 路径规则，按最长路径匹配，相同路径取最后一次 Lua 声明。`remove(path)` 在有效目标上排除其他模块对同一子树的贡献，再由所选后端处理 whiteout。
3. 计划就绪后，依次执行 `before()` 请求、常规模块挂载，再调用 `after()` 并执行其请求。全局挂载与常规模块挂载共用启动事务，执行阶段失败会触发回滚。[Lua 脚本示例与 API 说明](CUSTOM_MOUNT.md)

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

`status` 中的 `active_mounts` 合并、排序并去重各后端已确认的目标。API 1 的 `overlay_mount` 计入 `overlay_active_mounts`，`bind_mount` 计入 `magic_active_mounts`。两者都由 mountinfo 确认，使用父目录承载文件或缺失路径时记录实际挂载的目录。Magic Mount 创建的符号链接只计入操作统计。`vfs_active_mounts` 记录经提供程序回读确认的 VFS 目标，包括全局 `vfs_mount` 请求。

`vfs_modules` 列出本次启动使用 VFS 的模块 ID。全局 `vfs_mount` 归本次启动会话所有。`vfs_provider` 记录本次启动绑定的内核提供程序。

`storage_mode` 记录常规模块 OverlayFS 计划使用的 staging 后端，取值为 `tmpfs` 或 `ext4`。没有常规模块 OverlayFS 操作时记录 `none`。API 1 的 `overlay_mount` 使用独立的临时层。

# 分区布局诊断

`status` 的 `degraded` 表示存在已跳过的分区或操作；`partition_errors` 记录分区、原因及受影响模块，`operation_errors` 记录被拒绝的具体模块路径。正常分区仍可成功提交。
模块列表中的同名诊断字段仅在非空时输出，旧快照缺失字段按空列表读取。`is_mounted` 仍表示至少一个真实效果已确认，部分挂载会同时展示错误，不自动设置 `suggest_ignore`。

启动账本保存名单内全部分区的路径和身份。热加载核对相关分区；只有身份验证通过的本模块挂载可解释预期变化，未知覆盖不会被忽略。无布局基线的旧账本拒绝 load/reload，unload 仍按保存的规则与目标清理。
