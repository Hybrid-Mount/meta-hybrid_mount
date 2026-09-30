---
name: hm-runtime
description: 修改或排查 src/runtime/ 运行时层（boot、hot、ledger、transaction、rules、mounts、policy、lifecycle、device）时用：所有权账本阶段机、热挂载与 boot lock 竞态、事务回滚要清理的资源、状态快照契约。
whenToUse: 改动启动/热挂载/软重启生命周期、`runtime` 子命令、`run/runtime.json` 结构或回滚路径，或排查「启动成功但 runtime status 报不支持」「prepare-reboot 拒绝执行」时。
---

# 运行时层（`src/runtime/`）

`src/pipeline.rs` 决定「怎么挂」，`src/runtime/` 决定「这些资源归谁、什么时候可以放手」。
平台代码（boot/device/hot）只在 linux/android 编译，改动前先读 `docs/RUNTIME.md`。

## 模块职责（名称与真实文件一致）

| 文件 | 职责 | 公开入口 |
| --- | --- | --- |
| `mod.rs` | `runtime` 子命令分发；非 Linux/Android 直接报错 | `handle` |
| `boot.rs` | 一次启动的所有权：开始、暂存计划、提交、软重启清理 | `already_applied` `start` `stage_plan` `finish` `cleanup` |
| `hot.rs` | `status/load/reload/unload`、前置门禁、快照刷新 | `handle` `refresh_snapshots` |
| `device.rs` | boot_id + PID 1 mount namespace 作用域、keyring 访问 | `enter_init_namespace` `load` `Kernel::{open,inspect}` |
| `ledger.rs` | 账本 schema、阶段门禁、进程锁 | `Ledger` `ledger_for_boot` `save` `OperationLock::acquire` |
| `lifecycle.rs` | 纯判断，可在宿主机测试 | `reject_legacy_mounts` `owned_uids` `confirm_boot_rules` |
| `mounts.rs` | 真实挂载身份校验与最深优先卸载 | `capture` `validate` `cleanup` |
| `policy.rs` | 判断单个模块能否热挂载 | `plan_hot_module` `paths_overlap` |
| `rules.rs` | VFS 规则事务、外部冲突判定 | `preflight` `reconcile` `verify_owned` |
| `transaction.rs` | 单模块「UID 隔离 + 规则」事务与回滚 | `reconcile` |

## 启动流水线与运行时层的衔接点

1. 入口即 `enter_init_namespace()` + `OperationLock::acquire()`：所有写操作在 PID 1 命名空间且串行。
2. `boot` 子命令先查 `boot::already_applied()`（`phase == "ready"`）跳过重复钩子。
3. `boot::start()`：`require_clean` → 记录挂载基线 → 拒绝 legacy 挂载与未跟踪 VFS 规则 → `generation += 1` → 置 `applying`。
4. config → scan → VFS provider 探测 → plan（规划必须在 runtime 之前，见 `pipeline.rs`）。
5. `boot::stage_plan()` 写入 `pending_rules`、`isolated_uids`、`non_vfs_modules`，此后才存在归属关系。
6. state 阶段先落盘 `scan.ret` 与 `run/state.json`，让挂载失败时 WebUI 仍能看到计划。
7. storage → overlay → magic → vfs 的副作用全部注册进 `sys::transaction::MountTransaction`。
8. mountinfo 回读确认 → 写最终 state（`rollback_status=pending_commit`）→ `transaction.commit(disable_umount)` → `committed`。
9. `boot::finish(session, successful, targets)`：捕获新挂载 ID、KSU unmount 注册、回读 VFS 规则；全部通过才 `ready`，否则 `error`。
10. 挂载阶段失败时 `rollback_mount_pipeline` 反向执行清理闭包，并把 `failed_stage`/`failure_reason`/`rollback_status`/`leftover_mount_targets` 写进 state。

## 阶段机与竞态防护

- 合法 phase 只有 `clean | applying | ready | syncing | cleaning | error`，`ledger_for_boot` 会拒绝其它值。
- 锁是 `run/runtime.lock` 上的 `try_lock`；该文件永不删除（等待者必须锁同一 inode），进程退出自动释放。`/dev/hybrid_mount_single_instance` 只剩兼容性 rmdir。
- 账本按 `boot_id` + PID 1 namespace 作用域：物理重启作废旧挂载 ID，namespace 变化直接拒绝复用。
- `clean` 才允许完整流水线重建；`ready`/`syncing` 才允许热操作；`applying`/`error` 一律要求物理重启，**不要**加「尽力清理」分支。
- `hot::guard()` 在 `vfs_boot_guard` 存在时拒绝热操作，也不得代用户删除该标记。
- VFS 变更前 `VfsBootGuard::arm()`，正常返回由 Drop 清除；异常退出留下的 guard 是熔断机制，不是缺陷。

## 热挂载语义

- `status` 输出 `{supported, reason, generation, modules[{id,active,eligible,reason}]}`；任何前置失败折叠为 `supported:false`，命令本身仍返回成功。
- `load/reload/unload ID` 只接受纯 VFS 模块：`policy::plan_hot_module` 拒绝 skip-mount、黑名单、`remove` 标记、混合后端，以及与在用真实挂载路径重叠的规则。
- 已挂载的 Magic/Overlay 模块（`non_vfs_modules`）不能在线转 VFS，报错并要求重启。
- `load` 临时忽略 `disable` 标记而不删除文件；`unload` 依据账本，源文件已删也能卸载；`reload` 重新扫描并重注册（内核 pin 的是 inode）。
- 成功输出 `{"ok":true,"generation":N}`，之后必须 `refresh_snapshots` 同步 `state.json` 与 `scan.ret`；失败时调用方也要重新查询状态。
- `prepare-reboot`（`emulated-soft-reboot` 同义）只在身份校验通过时释放资源，`applying`/`error` 拒绝。

## 事务与回滚要清理什么

- 顺序不可颠倒：先 `add_uids` 并回读确认隔离 UID，再发布规则；只回滚本次新增的 UID。
- 规则删除按最深优先、写入按最浅优先；内容未变的规则也要重写以重 pin inode。
- 失败回滚：删除本次引入且已生效的规则并恢复 `before`；**只有规则回滚被验证干净才移除新 UID**，否则保留隔离并置 `error`。
- 真实挂载清理：先对全部目标 preflight（ID、device、fs_type、source、baseline、可见 `mnt_id`，外部或替换挂载一律拒绝），再最深优先 `umount2(DETACH)` 并逐个回读消失。
- 资源释放顺序：先卸载真实挂载，再释放 KSU unmount 注册（`release_mount_resources` 在失败时保留两者以便重试），反过来做会留下无法追溯的隐藏注册。
- `cleanup` 还需清 `rules`/`isolated_uids`/`pending_rules`/`non_vfs_modules`，把 `modules[].is_mounted` 置 false，最后刷新快照。

## 状态快照契约（写给 WebUI）

- `/data/adb/hybrid-mount/run/state.json`：`storage_mode`、`mount_point`、`overlay/magic/vfs_modules`、`overlay/magic/vfs_active_mounts`、`active_mounts`、`confirmed_active_mounts`、`mode_stats`、`mount_stats`、`failed_stage`、`failure_reason`、`rollback_status`、`leftover_mount_targets`、`vfs_error*`。
- `/data/adb/hybrid-mount/scan.ret`：`AppModule` 列表（`mode`、`is_mounted`、`enabled`、`blacklisted`、`rules` 等）。
- `active_mounts` 只由 mountinfo 确认的目标（VFS 由 provider 回读确认）合成；plan 的选择不能当作 `is_mounted`。
- `refresh_snapshots` 在 `clean` 时清空挂载类字段、`storage_mode` 置 `defs::NO_STORAGE_MODE`、`rollback_status` 置 `clean`，并同步 module.prop 描述。

## 常见坑

- 在宿主机跑挂载流水线：非 linux/android 直接返回错误；真实 mount、keyring、fdinfo 只能在设备上验。
- 把模块源目录当可写：一切 staging 写运行目录，runtime 层不得例外。
- 回滚漏清理：只删规则不清 UID、只卸载挂载不释放 KSU 注册、或失败时提前 `mounts.clear()`，都会让重试无法完成。
- 从挂载 `source` 推断所有权：归属只认「精确目标 + 新 `mnt_id` + 基线」，不要用 source/fs_type 猜。
- 快照与实际不一致：热操作后漏掉 `refresh_snapshots`，或在 `syncing` 阶段把失败当成功。
- 给 `Ledger` 加必填字段：旧 `runtime.json` 会直接解析失败（`{}` 本来就不是合法空账本，`ledger.rs` 测试有断言）；新字段一律 `#[serde(default)]`。
- 放宽冲突判定：`rules` 的路径冲突跨 UID 也成立（内核目录拓扑按名字索引），不要只比 `uid == 0`。

## 本地可跑的验证

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p hybrid-mount runtime::        # ledger/mounts/rules/transaction/lifecycle/policy 单元测试
sh tests/shell/boot_lock.sh                 # metamount.sh 必须透传 boot 失败（CI 覆盖）
sh tests/shell/runtime_hooks.sh             # emulated-soft-reboot.sh 必须透传 prepare-reboot 失败（仅本地）
cargo check -p hybrid-mount --target aarch64-linux-android
shellcheck module/*.sh tests/shell/*.sh
```

宿主机测试覆盖事务、回滚、外部冲突、legacy 判定与挂载栈清理；完整门禁见 `hm-verify`。

## 必须真机验证

- `runtime status|load|unload|reload|prepare-reboot` 全流程，以及 `/data/adb/ksud soft-reboot` 前后 `run/runtime.json` 的 phase/generation 变化。
- 实际文件可见性、`/proc/1/mountinfo`、provider 规则回读、`state.json` 四者一致。
- 外部写入者改规则或挂载、模块目录被替换/删除、apply 或 cleanup 被中断后的恢复路径。
- 并发发起两个 hot 操作时只有一个成功（进程锁），失败方退出码非零。

## 汇报格式

- 先给结论：改了哪个文件、动了哪个 phase / 字段 / 子命令。
- 列出实际执行的命令与结果；mount、keyring、软重启等未覆盖部分明确写成「未在设备验证」。
- 状态或回滚语义有变化时，给出对应单元测试名与真机复现步骤。
