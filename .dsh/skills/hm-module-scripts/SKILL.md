---
name: hm-module-scripts
description: 修改 Hybrid Mount 安装器与运行时 shell 脚本（module/*.sh、module/vfs/setup.sh、tests/shell/*.sh）前必读：各脚本的触发时机、可用变量与函数、退出码语义、平台裁剪规则和本地自测命令。
whenToUse: 要改 customize.sh、metainstall.sh、metamount.sh、boot-completed.sh、emulated-soft-reboot.sh、uninstall.sh、module/vfs/setup.sh，或调整 tests/shell/ 下的安装器自测时。
---

# Hybrid Mount 模块脚本

仓库共 13 个 shell 文件：`module/*.sh`（7 个）、`module/vfs/setup.sh`、`tests/shell/*.sh`（6 个）。它们跑在 Android 设备上、由 root 管理器以 `sh` 执行，**不是**本地工具脚本；改之前先确认自己在改哪一列。

## 执行时机与顺序

| 脚本 | 时机 | 执行者 | 关键职责 |
| --- | --- | --- | --- |
| `module/customize.sh` | 安装本模块时 | root 管理器安装器 **source**（非 exec） | 校验 KSU/APATCH 与 `$ARCH`；按平台裁剪 `lkm/`、`vfs/binaries/`；把当前架构二进制装到 `$MODPATH/hybrid-mount`；首装时铺 `config.toml` 并跑音量键向导、补种黑名单；设权限 |
| `module/metainstall.sh` | 安装/更新**任意普通模块**时（装本 metamodule 自身时不触发） | 官方安装在解包后 **source** | 导出 metamodule 标记；调用内建 `install_module`；为 `system/<分区>` 建顶层符号链接；删空的 `system/` |
| `module/metamount.sh` | 开机 post-fs-data 阶段最后一步（在所有 post-fs-data 脚本之后） | KernelSU/APatch，每次开机 | 委托 `hybrid-mount boot`；仅成功且有 `/data/adb/ksud` 时发 `ksud kernel notify-module-mounted`；退出码透传 |
| `module/boot-completed.sh` | 开机 boot-completed 阶段 | root 管理器 | 只删旧版遗留目录 `/dev/hybrid_mount_single_instance`，永远返回 0 |
| `module/emulated-soft-reboot.sh` | ksud 软重启（late-load）按 stage 名调用 | ksud | `exec hybrid-mount runtime prepare-reboot`，交给运行期账本清理 |
| `module/uninstall.sh` | 卸载本模块时 | root 管理器 | `rm -rf /data/adb/hybrid-mount`；不碰 `/data/adb/modules` |
| `module/vfs/setup.sh` | 手动：在内核树根执行（本地路径或 `curl \| bash`） | 开发者 | 把 `src/` 集成到 `fs/hybridmount/` 并改 `fs/Makefile`、`fs/Kconfig`；`--cleanup` 回滚；拒绝已集成 NoMount 的树 |

阶段名是 root 管理器的外部契约（KernelSU metamodule 文档：`metamount.sh` 在 post-fs-data 末尾、`metainstall.sh` 被 source、boot-completed 是独立阶段），仓库内只能从文件名与脚本注释确认；`metamount.sh` 是阻塞式的，别在里面加等待。

## 变量与路径约定

- `customize.sh`：`$MODPATH`、`$ARCH`（`arm64`/`arm`/`x64`/`riscv64`）、`$KSU`、`$APATCH`；函数 `ui_print`、`abort`、`set_perm`、`set_perm_recursive`；还依赖 `getevent`、`timeout`、`date`、`sed`、`grep`。
- `metainstall.sh`：`$MODPATH`、`$KSU`、`$APATCH`、`ui_print`、`install_module`；导出 `KSU_HAS_METAMODULE`、`KSU_METAMODULE=hybrid_mount`、`APATCH_HAS_METAMODULE`、`APATCH_METAMODULE=hybrid_mount`、`HYBRID_MOUNT=true`；用 `setfattr` 写 opaque 标记。
- `metamount.sh` / `emulated-soft-reboot.sh`：自己算 `MODDIR="${0%/*}"`，**不依赖** `$MODPATH`；`$MODDIR/hybrid-mount` 必须存在且可执行。
- `uninstall.sh` / `boot-completed.sh`：不读任何输入，路径硬编码。
- `module/vfs/setup.sh`：`$PWD` 必须是内核树根（有 `fs/` 或 `common/fs/`）；`$0` 含 `/` 才用本地 `src/`，管道执行时改从 `raw.githubusercontent.com` 下载；参数只有 `--cleanup`、`--help`。
- 稳定路径：二进制 `/data/adb/modules/hybrid_mount/hybrid-mount`；运行目录 `/data/adb/hybrid-mount`（`config.toml`、`scan.ret`、`run/`、`modules.img`、熔断标记）。

## 硬性约束

1. **解释器**：module 脚本一律 `#!/system/bin/sh`，`module/vfs/setup.sh` 是 `#!/bin/sh`。只写 POSIX sh，不要引入 bash 数组、`[[ ]]`、`${var^^}`。`local` 只出现在 `customize.sh` 且带 `# shellcheck disable=SC3043`，它依赖管理器 shell 支持。
2. **两种写入权限**：安装期脚本（`customize.sh`、`metainstall.sh`）可以写 `$MODPATH`；运行期脚本（`metamount.sh`、`boot-completed.sh`、`emulated-soft-reboot.sh`、`uninstall.sh`）绝不能写 `/data/adb/modules/*/system/**`，staging 一律落运行目录。
3. **退出码**：`metamount.sh` 必须原样抛出二进制退出码（`tests/shell/boot_lock.sh` 专测失败不被吞）；`boot-completed.sh` 必须永远返回 0（用 `|| :`），清理失败不得挡住开机；`emulated-soft-reboot.sh` 用 `exec` 继承退出码（`tests/shell/runtime_hooks.sh`）；`uninstall.sh` 显式 `exit 0`。
4. **stdout 纪律**：诊断与错误走 stderr（照 `metamount.sh`、`vfs/setup.sh` 的 `>&2` 写法）。二进制面向 WebUI 的 JSON 只走 stdout，包装脚本多打一行字就会让 `kernelsu.exec` 解析失败。
5. **metainstall 只做符号链接**：唯一允许的操作是 `ln -sf "./system/$partition" "$MODPATH/$partition"`，不得 `cp -a` + `rm -rf`、不得 `mv system/<partition>`；`ln` 之前必须先判 `-e`/`-L`，否则会往已存在的目录里再建一层链接。`handle_partition` 被刻意留成空实现、`mark_replace` 改写 opaque xattr，这两处是安装器动态解析的钩子，不要删。
6. **别手改生成物**：`module/module.prop` 被 `.gitignore` 忽略，由 `cargo xtask build` 生成到 `output/stage/module.prop`（`id=hybrid_mount`、`metamodule=1`，取自 `xtask/src/main.rs` 的 `MODULE_ID`）；打包时整份 `module/` 复制进 stage 再补二进制。改 id 或 metamodule 标记要改 xtask，并同步 `metainstall.sh` 的两个 `*_METAMODULE`。
7. `module/module_blacklist.toml` 头部写明由 `auto-blacklist-pr.yml` 自动维护，不要手工加条目。
8. `module/config.toml` 只是全新安装的种子：新增字段要同步 `src/config.rs` 的解析与默认值；向导只在 `/data/adb/hybrid-mount/config.toml` 不存在时运行，别改成覆盖已有配置。
9. **平台裁剪分支不能反**：`KSU=true` 时删 `lkm/`（改用官方 nuke ioctl），非 KSU 保留；`vfs/binaries/` 只在非 arm64 删，arm64 的 KernelSU 也保留（可经 ksud 加载），`vfs/src` 在任何平台都保留（GPL 完整性）。
10. **熔断标记不在脚本里**：`lkm_boot_guard`、`vfs_lkm_boot_guard` 由 `src/sys/lkm.rs` 的 `LoadAttemptGuard` 在 `insmod` 前写、正常返回后删。脚本侧与 LKM 相关的唯一落点是 `customize.sh` 的保留/裁剪分支，外加 `vfs/setup.sh` 对 NoMount 内核树的拒绝；`uninstall.sh` 删运行目录时会连带清掉标记。
11. `module/vfs/setup.sh` 只在没有 NoMount 的树里工作，Kconfig/Makefile 行用 `grep -F` 按字面量比较并追加；`--cleanup` 必须能原样回滚。改这些字面量要同步改 `tests/shell/vfs_setup.sh` 的断言。

## ShellCheck 与本地自测

```bash
shellcheck module/*.sh module/vfs/setup.sh tests/shell/*.sh
sh tests/shell/customize_lkm.sh
sh tests/shell/vfs_setup.sh
sh tests/shell/vfs_sources_digest.sh
sh tests/shell/vfs_sources_digest_test.sh
sh tests/shell/boot_lock.sh
sh tests/shell/runtime_hooks.sh   # lints.yml 目前没有这一步，改 emulated-soft-reboot.sh 时手动补跑
```

- `module/*.sh` **不匹配** `module/vfs/setup.sh`，必须显式点名（CI 里也有这句注释）。
- 保留文件里已有的抑制指令（`customize.sh` 的 `SC3043`、`setup.sh` 的 `SC2016`、`vfs_sources_digest.sh` 的 `SC2086`、`customize_lkm.sh` 的 `SC1090`），不要加全局 disable。
- 谁守护谁：`customize_lkm.sh` 测 `customize.sh` 的 KSU/架构分支（它把 `abort` 换掉、并在"二进制缺失"处提前结束，所以保留/裁剪分支必须留在二进制检查之前）；`boot_lock.sh` 测 `metamount.sh` 的退出码与 `boot` 子命令名；`runtime_hooks.sh` 测软重启委托；`vfs_setup.sh` 测集成、清理、NoMount 拒绝与下载失败不留残余。
- 改 `module/vfs/src/**` 必须重跑 `sh tests/shell/vfs_sources_digest.sh`；`.ko` 由 `kernel-module.yml` 重编，源码与产物是两个提交（见 `hm-vfs`、`hm-ci`）。
- 这些测试按 POSIX 写：在 WSL / Git Bash 里跑，不要在 Windows 上直接双击执行。

## 汇报格式

- 列出改动的脚本与一行动机；说明是否触及平台分支、退出码语义或生成物。
- 每条命令按 `命令 → 通过/失败` 列出；失败时给最小复现命令与原始错误片段。
- 明说是否补跑了上面 6 个自测脚本、是否需要重编 VFS 产物。
- 分工：本 skill 只管脚本自身机制；fmt/clippy/test/Android/WebUI 的全量门禁走 `hm-verify`。
