# Rust 后端完整审查报告

> 审查日期：2026-08-31  
> 审查提交：`070af486709b9b6504b58c2b01c0c49f49a067a3`（`dev`）  
> 修复状态更新：2026-09-02（HM-RUST-001、HM-RUST-002 已推送；HM-RUST-003 已修复）
> 审查范围：整个 Cargo workspace（`hybrid-mount`、`xtask`、`tools/notify`）  
> 规模：39 个 Rust 文件，约 15,508 行（含测试）

## 1. 结论摘要

当前后端的总体工程质量明显高于一般启动期挂载工具：规划与执行分层清楚，模块源目录按只读输入处理，错误路径有事务式回滚，子进程输出有界，配置/状态采用原子替换，并且已有较丰富的单元测试与故障注入。

本次审查最初确认了 **4 个 P1、8 个 P2、2 个 P3** 问题。截至 2026-09-02，**HM-RUST-001、HM-RUST-002、HM-RUST-003 已修复**，当前剩余 **1 个 P1、8 个 P2、2 个 P3**。发布前建议继续修复其余 P1：

1. ✅ OverlayFS 多级 staging 层覆盖问题已修复，并补充 0-256 层顺序/完整性测试。
2. ✅ ext4 容量规划已显式计入每个 shallow source 的再次物化，并补充 100 MiB 稀疏文件回归测试。
3. ✅ 启动配置改为 boot-only fail-closed；损坏、不可读、unsupported、dangling symlink 与缺失父目录均在扫描前终止。
4. LKM 启动熔断标记在 `insmod` 前没有同步父目录，掉电/内核崩溃后不能保证标记持久化，存在重复崩溃风险。

未发现 P0 问题。以当前提交直接发布的主要风险不是普通 Rust 内存安全，而是挂载层组合、磁盘容量、启动失败策略和设备级恢复语义。

## 2. 审查方法与验证结果

审查依据包括 Rust 所有权/错误处理/并发/性能/测试最佳实践，以及 rust-analyzer 的定义、引用和类型语义结果。重点检查了：

- 配置输入、模块扫描、规则优先级和目标路径映射；
- OverlayFS 分层、子挂载重建、shallow layer 和 staging 生命周期；
- Magic Mount 的 bind/move/replace/whiteout 语义；
- ext4/tmpfs、loop device、临时目录、SELinux xattr 和 LKM；
- 挂载事务、失败回滚、状态快照和 WebUI 兼容字段；
- 子进程超时、输出上限、退出码策略和命令查找；
- `xtask` 打包、Telegram 通知、依赖与 CI 可复现性。

### 已执行检查

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo test --workspace --all-targets --locked` | 通过：212/212（主 crate 207，notify 4，xtask 1） |
| `cargo test --workspace --doc --locked` | 通过；当前 0 个 doc test |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | 通过 |
| aarch64 Android Clippy（`-D warnings`） | 通过 |
| aarch64 / armv7 / x86_64 Android `cargo check --locked` | 全部通过 |
| Pedantic + Nursery + Perf Clippy | 完成；主要是文档、函数长度、`const fn`、命名等维护性提示，无新增编译级错误 |
| RustSec `cargo-audit 0.22.2` | 扫描 241 个锁定依赖、1,233 条 advisory，未发现已知漏洞 |
| `cargo tree --duplicates --locked` | 仅见 proc-macro 依赖树中的 `syn` 2/3 并存，未发现需立即处理的运行时重复依赖 |

### 运行链路

```text
config.toml
  -> 只读扫描 modules
  -> 共享 MountTree
  -> 冲突检测与 Overlay/Magic 规划
  -> 写入计划态 scan.ret / state.json
  -> 创建随机私有 staging
  -> OverlayFS（含 ext4/tmpfs 与 shallow layer）
  -> Magic Mount
  -> 提交 KSU try-umount 列表
  -> mountinfo 确认
  -> 写最终状态
  -> 清理或按 disable_umount 保留资源
```

## 3. 问题总表

优先级定义：P0 = 立即阻断/灾难性；P1 = 高风险、发布前应修复；P2 = 中风险、应进入近期迭代；P3 = 低风险诊断或维护性问题。

| ID | 级别 | 结论 | 主要位置 |
| --- | --- | --- | --- |
| HM-RUST-001 | P1（已修复） | 改为逐次折叠并立即插回 staging 输出，保留全部层及顺序 | `src/overlayfs/overlayfs.rs:122-136, 250-264, 479-563` |
| HM-RUST-002 | P1（已修复） | 容量输入保留重复 shallow source，按实际再物化次数计费 | `src/pipeline.rs:319-388, 1025-1043`; `src/storage/ext4.rs:165-251, 459-485` |
| HM-RUST-003 | P1（已修复） | boot loader 仅对真正缺失的配置使用默认值，其余错误在扫描前终止并写失败快照 | `src/config.rs:191-264`; `src/pipeline.rs:601-627`; `src/state.rs:141-149, 312-329` |
| HM-RUST-004 | P1 | LKM 熔断标记缺少父目录 fsync，崩溃后可能丢失 | `src/sys/nuke.rs:168-220` |
| HM-RUST-005 | P2 | 跟随目录符号链接取 stat，却不跟随地读取 SELinux xattr | `src/sys/fs.rs:346-390`; `src/magic_mount/exec.rs:489-520`; `src/utils/mod.rs:31-72` |
| HM-RUST-006 | P2 | Overlay 子挂载检查用 `is_dir/exists` 跟随模块符号链接 | `src/overlayfs/overlayfs.rs:333-393` |
| HM-RUST-007 | P2 | 成功挂载目标反推模块时漏掉 reroute/结构父节点的子树贡献 | `src/state.rs:389-429`; `src/pipeline.rs:1128-1189` |
| HM-RUST-008 | P2 | LKM fallback 在首个可执行候选未产生效果时提前结束 | `src/sys/nuke.rs:111-160` |
| HM-RUST-009 | P2 | `mountsource` 未校验，软重启命令可卸载无关的同 source 挂载 | `src/config.rs:86-103, 288-293`; `src/sys/mount.rs:139-184` |
| HM-RUST-010 | P2 | 清理 `mount_error` 会递归删除同名目录，而非只 unlink 标记文件 | `src/state.rs:618-653`; `src/sys/fs.rs:90-97` |
| HM-RUST-011 | P2 | 原子写临时名为 PID+进程内序号，崩溃残留可阻塞后续启动写入 | `src/sys/fs.rs:41-75` |
| HM-RUST-012 | P2 | 扫描记录的节点类型在 staging 时未重新核对，存在 scan/use 竞态 | `src/scanner.rs:280-301`; `src/sys/fs.rs:138-179` |
| HM-RUST-013 | P3 | Magic 目录挂载和 whiteout 没有进入最终统计 | `src/magic_mount/exec.rs:108-116, 193-350, 412-430`; `src/pipeline.rs:56-74` |
| HM-RUST-014 | P3 | Magic-only 运行仍显示配置中的 `ext4/tmpfs` 为实际存储模式 | `src/state.rs:264-303`; `src/pipeline.rs:335-375`; `src/module_status.rs:45-53` |

## 4. 详细发现

### HM-RUST-001（P1）：多次 Overlay staging 折叠会丢失 lower layers

**状态：已于 2026-09-01 修复。**

修复后的 `collapse_staging_layers` 每次只从当前真实层集合拆出低优先级尾部，成功生成 staging layer 后立即插回，再决定是否进行下一轮。运行时不再预计算引用陈旧原始输入的 chunk。新增测试覆盖 127/128 层边界，并对 0-256 层验证最终覆盖集合与顺序完全不变。

**触发条件**

原实现中，单个目标的 `module lowerdirs + stock lowest` 达到 127 层时，最终 mount 仍可能保留 65 层；达到 128 层时会进入错误的多次折叠并丢层。

**证据**

原 `plan_staging_chunks` 在纯 `Vec<String>` 上预先计算所有 chunk。它每轮只从模拟数组尾部 `drain`，却没有把“上一轮生成的 staging layer”插回模拟数组。运行阶段随后按预计算的 `remaining_layers` 截断真实数组并插入 staging 路径，但下一 chunk 仍只引用原始路径，不引用前一 staging。

以 128 个总层 `L0..L127` 为例：

- 第一次折叠 `L65..L127`，生成 `S0`；
- 第二个预计算 chunk 是 `L2..L64`，不包含 `S0`；
- 最终只剩 `L0, L1, S1(L2..L64)`；
- `L65..L127` 共 63 层静默消失。

这不是单纯的上限错误：挂载可以成功，但最低优先级模块和 stock 内容会缺失，最终合并视图错误。

**已实施修复**

- 删除了“预计算全部 chunk”的模型，改为运行阶段闭环折叠。
- 每个新 staging 输出会参与下一轮输入，因此不会成为孤立、丢失的挂载层。
- 边界测试新增 126、127、128 层，并验证每一步和最终 mount 都不超过 64 层。
- 顺序/完整性测试覆盖 0-256 层，验证折叠前后展开结果完全相同。

### HM-RUST-002（P1）：ext4 image 对 shallow overlay 稳定低估容量

**状态：已于 2026-09-01 修复。**

启动阶段现在只构建一次 `OverlayExecutionPlan`，容量输入由“参与 Overlay 的模块根”与“执行计划中的每个 shallow source”组成。shallow 路径刻意不去重，因为每次出现都代表 prepared tree 之外的一次真实复制。完成 staging 后，同一份执行计划的源路径被重映射到 prepared tree，再直接交给挂载阶段，避免容量计划与执行计划漂移。

**触发条件**

使用 ext4 staging，且存在文件级 overlay 或缺失目录 reroute 到 shallow layer，尤其是 APK、字体包、资源包等较大文件。

**证据**

原容量规划在挂载 ext4 前，只对 `config.moduledir` 的源树计算一次逻辑大小。随后同一选中文件会：

1. 在 `stage_overlay_tree` 中复制到 ext4 prepared tree；
2. 在 `mount_overlay_files` 中再次复制到 ext4 内的独立 shallow layer。

规划公式只有 `1.25 * source_bytes + 16 MiB`。因此最小、只含一个 shallow 文件的模块会明显不足：

| shallow 源文件 | 计划 image | 两份数据的最低需求 | 未计缺口 |
| ---: | ---: | ---: | ---: |
| 50 MiB | 80 MiB | 100 MiB | 20 MiB |
| 100 MiB | 144 MiB | 200 MiB | 56 MiB |
| 256 MiB | 336 MiB | 512 MiB | 176 MiB |

原实现还存在两个相反方向的问题：扫描整个 `moduledir` 会把 disabled、blacklisted、Magic-only 和非挂载文件计入，通常过度分配；若 `moduledir` 本身是目录符号链接，`symlink_metadata` 又只计一个 inode，可能严重低估。

**影响**

第二次 `copy_prepared_entry` 返回 `ENOSPC`，整个挂载事务回滚；文件越大越容易稳定复现。

**已实施修复**

- 初始容量只扫描实际参与 Overlay 的模块根，避免自定义 `moduledir` symlink 根只计一个 inode，也排除非模块目录。
- `OverlayExecutionPlan` 中每个 shallow source 会作为额外 sizing path 再统计一次；文件级与缺失目录 reroute 都覆盖。
- `calculate_total_size` 不再用 `Path::exists` 预过滤根路径，broken symlink 等仍按实际 inode 语义计数。
- 100 MiB 稀疏源重复一次后按 200 MiB 数据需求计算，最终 image 计划从原先 144 MiB 提升为 268 MiB。
- 仍保留 25% + 16 MiB 余量；未来可进一步改为逐节点精确模型或同文件系统 hardlink，以减少保守过分配。

### HM-RUST-003（P1）：启动配置错误时采用危险默认值继续挂载

**状态：已于 2026-09-02 修复。**

新增 boot-only `Config::load_for_boot`：只有父目录真实可访问且最终配置文件确实不存在时才加载默认值和黑名单；损坏 TOML、不支持的全局模式、读取错误、dangling symlink、缺失/异常父目录及 blacklist 错误都会保留结构化错误并在模块扫描前终止。`load_or_default` 仍供 WebUI 显示、配置修复和状态查询使用，原容错行为保持不变。

**触发条件**

`config.toml` 存在但 TOML 损坏、不受支持或读取失败（权限、I/O 等）。

**证据**

原实现中，`Config::load_or_default` 对上述错误记录 warning 后返回 `Config::default()`；启动流水线直接使用它扫描、规划并挂载。缺失配置使用默认值是合理的首次启动语义，但“已有配置损坏”和“配置从未存在”被赋予了近似相同的执行结果。

**影响**

- 用户原先的模块级/路径级 `ignore`、Magic 规则和自定义模块目录同时失效；
- 原本为避免 bootloop 而忽略的模块可能重新按默认 OverlayFS 参与启动；
- WebUI 只看到默认配置，不能从当前 `Config` 判断这次启动实际发生过解析错误。

独立 blacklist 已选择 fail-closed，因此主配置的 fail-open 行为与项目已有安全策略不一致。

**已实施修复**

- 无参数启动只调用严格 `load_for_boot`；查询和修复命令继续调用容错 `load_or_default`。
- 配置错误时创建全新的空 `RunState`，记录 `failed_stage=config`、`failure_reason` 和 `rollback_status=clean`，不继承旧活动挂载。
- 同时把 `scan.ret` 原子替换为空数组，避免上一轮模块的 `is_mounted=true` 缓存残留。
- 7 个 strict loader 测试覆盖有效、真正缺失、损坏、不支持、不可读、dangling symlink 和缺失父目录；原有 4 个容错加载与 6 个 blacklist 测试继续通过。

### HM-RUST-004（P1）：LKM 启动熔断标记没有完整落盘保证

**触发条件**

非 KSU 环境选择兼容 LKM，设备在 `insmod` 期间内核崩溃或突然断电。

**证据**

`LkmAttemptGuard::arm_at` 使用 `create_new` 创建标记，写入内容并 `marker.sync_all()`，随后立即允许 `insmod`。它没有打开并 fsync 标记文件的父目录。文件 fsync 不能跨文件系统地保证“新目录项”已经持久化；这正是 `sys::fs::atomic_write` 在 rename 后额外同步父目录的原因。

**影响**

若内核崩溃后新建目录项丢失，下次启动看不到 `lkm_boot_guard`，会再次加载同一不兼容 LKM，熔断机制无法阻止重复 boot crash。

**建议**

- 在 `marker.sync_all()` 后、调用任何 `insmod` 前，对父目录执行 `File::open(parent)?.sync_all()`。
- 保留当前 `create_new` 的排他语义。
- 删除标记后同步父目录属于可选优化；删除未持久化只会安全地 fail-closed，不会造成重复崩溃。

### HM-RUST-005（P2）：符号链接目录的 SELinux 元数据来源不一致

**触发条件**

真实挂载目标是指向目录的符号链接，例如源码注释列出的 `/system/media` 类布局。

**证据**

`directory_metadata` / `tmpfs_skeleton` 使用 `fs::metadata` 跟随最终 symlink 读取目标目录的 mode/uid/gid；随后 `lgetfilecon` 使用 `lgetxattr`，明确不跟随最终 symlink，因此读取的是链接 inode 的 SELinux context。最终 staging 目录混合了“目标目录权限 + 符号链接标签”。链接无对应 xattr 时还会直接失败。

**影响**

可能导致整个 Overlay/Magic 目标使用错误标签，或在本应支持的 symlink 目录布局上中止挂载。代码自身已注明错误 layer label 可能使整个 Android 分区不可访问。

**建议**

- 对“需要跟随的 stock directory”先取得稳定解析路径/目录 fd，再从同一对象读取 stat 与 SELinux xattr。
- 保留模块源 symlink 的 no-follow 语义；不要全局把 `lgetfilecon` 改成 follow，应提供两个明确 API。
- 测试链接 inode 与目标目录使用不同标签、链接无 xattr 两种情况。

### HM-RUST-006（P2）：Overlay 子挂载重建会跟随模块 symlink

**触发条件**

模块在某个 stock 子挂载点位置贡献 symlink，且该 symlink 指向一个目录。

**证据**

`mount_overlay_child` 使用 `Path::exists()` 和 `Path::is_dir()` 检查 staged module path；两者都会跟随 symlink。之后把原 symlink 路径作为 lowerdir 交给内核，内核解析后实际使用链接目标目录。

**影响**

这与 scanner/staging 的“symlink 不跟随”契约冲突。绝对链接可把 staging 之外的目录意外纳入 lowerdir；相对链接也可能改变本应作为 symlink 覆盖项的语义。

**建议**

- 使用 `symlink_metadata` 分类；只有真实目录才能成为 child lowerdir。
- symlink、普通文件、whiteout 应走显式的“覆盖/不重建子挂载”分支，不应通过 `is_dir` 猜测。
- 增加 child mount point 上 symlink-to-dir、broken symlink 和绝对 symlink 测试。

### HM-RUST-007（P2）：执行目标到模块的反向归因不完整

**触发条件**

- Overlay 目录不存在，被 reroute 到最近存在的祖先；或
- Magic 通过结构父目录的一次 tmpfs move 承载子孙节点（尤其是空目录、空 `.replace` 目录）。

**证据**

执行器报告的是实际 `mount_target`，而 `mounted_module_ids_for_snapshot` 仍用计划 map 的原始 key 判断 shallow，再对实际 target 只查询当前节点来源。reroute 后实际 target 往往只是没有直接 `sources` 的结构节点，真正贡献位于子树中。

**影响**

mountinfo 和 `active_mounts` 显示成功，但 `scan.ret.is_mounted` 仍为 `false`；WebUI、故障诊断和用户判断互相矛盾。

**建议**

- 执行计划为每个实际 target 携带明确的 `module_ids`，执行成功时直接返回，不要事后从字符串路径反推。
- 至少对 rerouted Overlay 与 Magic Move/Replace 使用 subtree attribution。
- 增加“缺失目录 reroute + 成功 target”和“新建空 Magic 目录”测试。

### HM-RUST-008（P2）：LKM 候选 fallback 提前终止

**触发条件**

第一个存在的 `/system/bin/insmod` 能执行，但因实现/参数/策略差异未移除目标 procfs 节点；后续 BusyBox applet 本可成功。

**证据**

由于成功语义以 procfs 节点消失为准，进程退出码被设为 `Any`。但 `run_command` 返回 `Ok` 且节点仍存在时，函数立即 `return Err`，不再尝试剩余候选。反过来，`run_command` 返回 timeout/drain error 时又没有先复查权威副作用，可能在节点已经消失后重复尝试下一候选。

**建议**

- 每次尝试后首先检查 procfs 节点；消失即成功，与进程结果无关。
- 节点仍存在时，把 status/stderr/error 追加到 `attempts` 并继续下一个候选。
- 只有候选耗尽后再返回聚合错误。

### HM-RUST-009（P2）：软重启卸载范围受未校验 mount source 控制

**触发条件**

配置把 `mountsource` 设为通用 source 名（例如 `tmpfs`、`none` 或其他服务正在使用的值），随后调用 `emulated-soft-reboot`。

**证据**

配置 schema/patch 接受任意字符串。软重启按 mountinfo 的 `mount_source == configured source` 选择所有非 OverlayFS 挂载，不再限定为本项目记录的目标。

**影响**

可能 lazy-unmount 与 Hybrid Mount 无关的系统或其他模块挂载。默认 `KSU/APatch` 降低了正常路径风险，但 WebUI 可持久化任意值。

**建议**

- `mountsource` 改成受约束 enum，或至少拒绝内核通用 source 名、空值、控制字符和过长值。
- 更可靠的做法是从本项目状态/marker 读取精确 mount ID/target，并在执行前用 mountinfo 重新验证 source 与 fs type。
- 若该命令确实旨在清理整个 root manager 的 Magic Mount，应在 CLI 文档和命令名中明确这一破坏性范围。

### HM-RUST-010（P2）：清理错误标记可能递归删除目录

**触发条件**

模块根下存在大小写任意的 `mount_error` **目录**，而不是普通标记文件。

**证据**

`clear_mount_error_markers` 只按名称匹配，然后调用通用 `remove_path`；后者遇到目录会 `remove_dir_all`。

**影响**

用户执行“清理错误标记”会递归删除该目录全部内容。即使名称按约定保留，标记 API 也不应把异常类型升级为递归数据删除。

**建议**

- 通过 `symlink_metadata` 只接受普通文件；symlink 可选择只 unlink 链接本身。
- 目录或特殊文件记录 warning 并跳过。
- 增加同名目录、symlink、FIFO 的回归测试。

### HM-RUST-011（P2）：原子写 crash residue 可造成持续写失败

**触发条件**

进程在临时文件 `create_new` 后、rename 前崩溃，之后某次写入进程复用了同一 PID，并且进程内序号从 0 重新开始。

**证据**

临时名固定为 `.<name>.<pid>.<sequence>.tmp`，序号是进程内全局原子计数。崩溃不会执行清理；下一进程的序号重新从 0 开始。遇到残留文件时只尝试一次 `create_new`，不会换名重试。

**影响**

`scan.ret`、`state.json` 或配置保存可在启动期直接返回 `AlreadyExists`。若早期启动 PID 稳定复用，同一残留文件可让后续每次启动都失败。

**建议**

- 使用 `getrandom` 生成临时后缀并在少量碰撞时重试；或使用安全的同目录 tempfile API。
- 对符合严格自有命名、uid 和类型条件的旧临时普通文件做受控清理。
- 保留 file fsync + rename + parent fsync；设置旧权限后应再次同步 inode 元数据。

### HM-RUST-012（P2）：scanner 与 staging 之间存在类型竞态

**触发条件**

扫描完成后、staging 复制前，模块源节点被另一个进程替换（例如普通文件换成 symlink，目录换成文件）。

**证据**

scanner 将 `NodeFileType` 存入计划。staging 虽重新读取 `symlink_metadata`，但随后按计划中的旧 `source.file_type` 决定 `fs::copy`、`read_link` 或 `create_dir_all`，没有比较新 metadata 的真实类型。

**影响**

普通文件若被换成 symlink，`fs::copy` 会跟随新链接读取目标；类型变化也可能形成部分 staging 后才失败的非确定行为。这破坏“扫描的只读快照”假设。

**建议**

- 复制前验证实际类型、设备号和关键 identity 与扫描记录一致，不一致即中止整个模块/计划。
- 更强方案是 scanner 持有 `openat2`/目录 fd 建立的 no-follow 句柄，执行阶段基于 fd 而不是重新解析路径。
- 文档若把模块目录视为可信且启动期间不可变，也应由文件锁/权限或明确前置条件保证，而不是隐含假设。

### HM-RUST-013（P3）：挂载统计遗漏 Magic 目录与 whiteout

**证据**

- whiteout 分支返回成功并记录模块，但没有增加 `ignored_files`；该字段没有其他生产写入点。
- `pipeline_stats` 只接收 Magic 文件和 symlink 数量，不接收 `Move/Replace` 目录挂载数。

**影响**

状态中的 `ignored_entries` 恒为 0；Magic 目录挂载存在于 `active_mounts`，却不计入 `total_mounts/successful_mounts`。状态字段之间不守恒。

**建议**

按 `MagicOperation` 统一计数：bind、move、replace、symlink、whiteout 分别维护，再由一个函数生成兼容统计字段。测试应断言 whiteout 和目录 move/replace。

### HM-RUST-014（P3）：Magic-only 运行显示不存在的存储后端

**证据**

计划态把 `config.overlay_mode` 直接写入 `state.storage_mode`。没有 Overlay 操作时 storage phase 跳过，这个值不会改成 `none`；模块描述随后把除 `tmpfs` 外的值都渲染成 `Ext4`。

**影响**

Magic-only 或空计划仍显示“Ext4 运行中”，容易误导设备排障。

**建议**

区分 `requested_overlay_mode` 与 `actual_storage_mode: none|tmpfs|ext4`；模块描述只在实际创建 Overlay storage 时显示该后端。

## 5. 工程质量与测试缺口

### 5.1 值得保留的设计

- workspace lint 已禁止生产 `unwrap/expect/todo/unimplemented/dbg!`；本次标准 Clippy 全部通过。
- 统一 `CommandSpec` 不拼 shell 字符串，参数分离，stdout/stderr 使用有界 head+tail 并继续 drain，可避免常见的注入、OOM 和管道死锁。
- 配置与状态使用临时文件、file fsync、rename、parent fsync；除 HM-RUST-011 外，崩溃一致性思路正确。
- `MountTransaction` 按逆序清理并聚合所有失败，流水线还用 mount ID 对比 baseline，明显优于只依赖 Drop 的 best-effort 回滚。
- 临时目录使用内核随机数、22-30 位名称、`0700` 和 `create_dir` 排他创建。
- planner 在执行前检测跨后端文件/类型/`.replace` 冲突；模块 ID 使用 newtype 在反序列化边界验证。
- 关键线格式有 snapshot 风格断言；配置、planner、state、fault injection、process runner 的测试覆盖较完整。
- 项目自身生产代码没有直接 `unsafe` 块；平台级 unsafe 被封装在依赖/API 边界，项目内出现的 `unshare_unsafe` 仅用于 Linux 测试。

### 5.2 仍需补齐的测试

1. **真实挂载测试可能“绿色跳过”**：Linux 测试在 `unshare`/mount 不可用时直接 `return`，CI 仍计为通过。应增加具备 `CAP_SYS_ADMIN` 的专用 job；该 job 若不能建立 mount namespace，应失败而不是静默通过。
2. **缺少真实内核的多级 staging 集成测试**：0-256 层的纯逻辑覆盖/顺序测试已补齐，但仍需在支持的内核上验证多次 OverlayFS staging mount。
3. **缺少 ext4 空间集成测试**：重复 shallow source 与 100 MiB 稀疏文件容量单测已补齐，但尚未在真实 ext4 loop 中执行 prepared + shallow 两次物化。
4. **缺少 symlink + SELinux 真机测试**：主机测试无法验证 `security.selinux`、mount target symlink、tmpfs xattr 和 Android policy。
5. **缺少 KSU/APatch/LKM 故障测试**：boot guard、候选 fallback、ioctl、insmod 崩溃只能在隔离测试设备或虚拟内核环境验证。
6. **doc test 为 0**：内部 binary crate 不要求大量 rustdoc，但 `notify` 的公开 API 至少应补 `# Errors` 和一个可运行示例。
7. **缺少 promoted partition 双实体布局测试**：同一模块同时含真实 `system/vendor/**` 与 `vendor/**` 时，两条来源映射同一 target，当前 `BTreeMap<ModuleId, PathBuf>` 只保留一个 lowerdir；应明确冲突或合并语义并加测试。

### 5.3 可维护性与供应链

- `ErrorClass` 设计合理，但生产路径仍有约 100 处 `Error::msg(format!(...))`；这会擦除原始 errno/可重试分类，而当前分类 API 又没有生产消费者。建议逐层迁移为 `ContextError + CausalError`，并让状态/日志实际消费分类。
- `pipeline.rs`、`plan/mod.rs`、`state.rs` 职责较重。建议围绕“纯计划”“资源获取”“执行回执”“状态投影”拆分，而不是仅按行数抽 helper。
- `rust-toolchain.toml` 使用浮动 `nightly`，CI 容器使用 `:latest`，两项会使同一提交的构建结果随时间变化。建议固定 dated nightly 与 CI image digest。
- 两个 git 依赖在 `Cargo.lock` 中锁定 commit，但 manifest 未写 `rev`；发布构建/CI 多处也没有 `--locked`。建议 manifest 固定审核过的 rev，所有 release/check 命令加 `--locked`。
- RustSec 本次未发现已知漏洞；这不覆盖 git 依赖源码审计、LKM ABI 安全或未来 advisory。

## 6. 建议修复顺序

### 第一批：发布阻断项

1. ✅ HM-RUST-001 已完成：多级 staging 改为逐次闭环折叠，并增加覆盖集合/顺序回归测试。
2. ✅ HM-RUST-002 已完成：容量输入消费执行计划并按 shallow source 的重复物化次数计费。
3. ✅ HM-RUST-003 已完成：boot-only 严格加载、失败状态持久化和旧模块快照失效均已实现。
4. 在任何 LKM 加载前完整持久化 boot guard（HM-RUST-004）。

### 第二批：设备可靠性

1. 统一 symlink 的 stat/xattr follow policy（HM-RUST-005、006）。
2. 让执行器直接回传 target -> module IDs（HM-RUST-007）。
3. 修复 LKM fallback 聚合循环（HM-RUST-008）。
4. 收紧软重启卸载范围和 `mountsource` schema（HM-RUST-009）。
5. 收紧 marker 删除与 scan/use identity（HM-RUST-010、012）。

### 第三批：诊断与工程化

1. 随机化原子写临时名并处理 crash residue（HM-RUST-011）。
2. 修正统计/实际存储模式（HM-RUST-013、014）。
3. 增加 privileged Linux 挂载 CI、Android 真机回归矩阵、dated toolchain 和 locked release build。

## 7. 审查边界

- 本次在 macOS 主机完成单测、Clippy、RustSec 和 Android 交叉编译检查；Android 产物没有在当前环境执行。
- 真实 `mount(2)`、loop device、SELinux、KernelSU/APatch ioctl、LKM ABI 和 boot crash 恢复必须在受支持设备验证。
- 本报告审查了 Rust workspace 及其直接构建/通知边界；WebUI、shell 安装脚本和内核模块 C 源码不是本报告的完整审查对象。
- 严格 Clippy 的 pedantic/nursery 提示未逐条列为缺陷；纯格式、`const fn`、反引号和轻微 clone 建议不影响当前正确性。
