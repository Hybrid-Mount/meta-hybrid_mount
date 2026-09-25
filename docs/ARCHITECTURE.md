# Hybrid Mount 架构

本文描述 v6 `dev` 当前实现。历史重构阶段、orphan 分支操作和已完成的迁移清单不再作为运行契约。

## 维护原则

- `/data/adb/modules/<id>/system/**` 始终是只读输入，安装、扫描、规划和执行阶段都不得移动、合并或删除其中内容。
- v4.2.0 中经过长期实机验证的必要设计可作为回归修复的实现基线；直接移植时必须保留关键语义、许可证和原贡献者归属。
- Magic Mount 行为继续与上游 `meta-magic_mount-rs` 的既定契约核对；混合 planner 允许 Overlay 与 Magic 两个真实挂载后端共享普通结构目录，同时保证同一实际文件只进入一个后端。
- 配置字段、CLI 输出和下列稳定路径是 module 脚本与 WebUI 之间的兼容接口，不能只为品牌整理而改名。
- 仓库保留完整 Git 历史和作者信息；完成后的阶段计划可以删除，但不能据此压平或重写贡献记录。

## 启动流水线

```text
module/metamount.sh
  → hybrid-mount boot（运行锁与本轮去重）
  → 读取 config.toml
  → 只读扫描模块与受管分区，识别文件/目录/符号链接/.replace/whiteout
  → 生成一棵带 overlay / magic / vfs / ignore 标注的共享节点树
  → 从共享树派生互斥的 OverlayFS 操作
  → 预写 scan.ret 与 run/state.json
  → 准备临时 staging
  → 从同一棵树物化并执行 OverlayFS，再执行 Magic Mount
  → 再执行 VFS 注入（HM 自有的 hybridmount Provider）
  → 提交 KernelSU try-umount 列表
  → 更新状态快照并清理临时资源
```

流水线不移动、合并或删除 `/data/adb/modules/<id>/system/**`。所有 staging 写入随机临时挂载或 `/data/adb/hybrid-mount` 下的持久资源。若挂载阶段失败，已生成的规划快照仍供 WebUI 显示所选后端和失败状态。

## 代码分层

- `src/config.rs`：TOML schema、默认值、升级兼容与 WebUI patch 持久化。
- `src/scanner.rs`：只读读取模块元数据、状态标记，并识别所有可挂载节点类型。
- `src/mount_tree.rs`：OverlayFS、Magic Mount 与 VFS 唯一共享的节点树、模块贡献、结构父链与后端标注。
- `src/plan/`：应用“路径规则 > 模块默认值 > 全局默认值”，在共享树上检测跨后端冲突并派生 Overlay 操作。
- `src/overlayfs/`：从共享树物化文件、目录、符号链接、opaque `.replace` 与 whiteout，随后执行 64 层分段、子挂载重建与文件级 shallow layer。
- `src/magic_mount/`：直接消费共享树，执行 tmpfs skeleton、mirror、bind、`.replace` 与 whiteout 语义；不再二次扫描模块目录。
- `src/vfs/`：HM 自有的 VFS 后端（`hybridmount` 模块）。`rule.rs` 把共享树映射为规则，`protocol.rs`
  编解码 HM 专用 wire protocol，`sys.rs` 通过 keyring `add_key` 发送并维护页对齐缓冲，
  `backend.rs` 只绑定 key type `hybridmount` 的 Provider，并在检测到外来 NoMount 时拒绝并存，
  `lkm.rs` 在内核未内建时从 `vfs/binaries/` 加载精确匹配的预编译模块，
  `exec.rs` 应用规则并统计。
- `src/storage/`：tmpfs 或 ext4 loop staging；ext4 镜像位于 `/data/adb/hybrid-mount/modules.img`。KernelSU 安装会删除 `lkm/` 并只使用官方 sysfs nuke ioctl；APatch 等非 KSU 安装保留 LKM，ext4 挂载后由 `src/sys/nuke.rs` 默认选择精确匹配的预编译版本。
- `src/pipeline.rs`：启动顺序、资源生命周期、卸载注册与失败状态持久化。
- `src/runtime/`：启动与热操作共享的所有权账本、规则事务、挂载身份校验和软重启清理；详见 [RUNTIME.md](RUNTIME.md)。
- `src/state.rs`：`scan.ret`、`run/state.json` 以及 WebUI 所需查询命令。
- `src/sys/`、`src/utils/`：挂载、文件系统、随机临时目录、SELinux xattr 与 KernelSU 接口。
- `webui/`：Vue 3 双界面，通过 `kernelsu.exec` 调用同一个 Rust 二进制。
- `xtask/`：WebUI 构建、Android 交叉编译、module.prop 生成和 ZIP 打包。

## 稳定路径与数据

- 模块目录：`/data/adb/modules/hybrid_mount`
- 二进制：`/data/adb/modules/hybrid_mount/hybrid-mount`
- 配置：`/data/adb/hybrid-mount/config.toml`
- 模块快照：`/data/adb/hybrid-mount/scan.ret`
- 运行所有权：`/data/adb/hybrid-mount/run/runtime.json`（boot ID 与 PID 1 mount namespace 作用域）
- 启动及最近热操作状态：`/data/adb/hybrid-mount/run/state.json`
- ext4 staging 镜像：`/data/adb/hybrid-mount/modules.img`
- 可选 LKM：`/data/adb/modules/hybrid_mount/lkm/binaries/*.ko`
- LKM 启动熔断标记：`/data/adb/hybrid-mount/lkm_boot_guard`

这些路径属于安装、WebUI 和启动脚本之间的兼容接口，不应仅为品牌或目录整理而改名。

LKM 子树是独立标识的 GPL-2.0-only 组件，核心 userspace/module 仍为 GPL-3.0-only，WebUI 仍为 Apache-2.0。预编译 LKM 只按受支持的内核线与 Android/GKI 版本做精确候选选择，不能替代实机 ABI 校验；KernelSU 安装不保留这些文件，非 KSU 环境默认尝试匹配项。

发布 ZIP 在安装前包含 `binaries/hybrid-mount-arm64`、`binaries/hybrid-mount-armv7`、`binaries/hybrid-mount-x86_64` 和 `binaries/hybrid-mount-riscv64`。`customize.sh` 只复制当前架构对应的文件到上述稳定二进制路径，随后删除安装目录中的 `binaries/`，设备上没有第二套常驻可执行文件。

挂载所需的临时目录使用内核随机生成的 22–30 位字母数字名称和 `0700` 权限，依次尝试 `/mnt`、`/mnt/rw`、`/tmp`。名称不包含项目、PID 或时间戳特征；正常结束时递归清理，仅在 `disable_umount = true` 明确保留挂载时保留对应路径。

## CLI 契约

设备上的可执行文件通常是 `/data/adb/modules/hybrid_mount/hybrid-mount`。无参数时直接运行启动挂载流水线；命令失败会写入 stderr 并以非零状态退出。

| 命令 | 参数 | 输出与行为 |
| --- | --- | --- |
| *(无参数)* | 无 | 执行完整启动挂载流水线。 |
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

WebUI 不持有第二套业务协议：配置与状态请求都映射到以上命令。状态是启动快照，不是 daemon 提供的实时流。

`status` 中的 `active_mounts` 是 OverlayFS、Magic Mount 与 VFS 成功目标合并、排序、去重后的兼容字段；`overlay_active_mounts`、`magic_active_mounts` 与 `vfs_active_mounts` 保留分后端明细。Magic Mount 只把成功的文件 bind 目标和目录 mount-move 目标计入活动挂载点，符号链接创建仍只进入操作统计，不伪装成挂载点。OverlayFS 与 Magic Mount 的目标必须经 mountinfo 确认；VFS 注入点不是内核挂载，改由 `apply_vfs_phase` 的 Provider 读回确认，读回失败的批次会被整体丢弃，因此不会进入 `active_mounts`。

两类的 mountinfo 字段都属于对外契约。OverlayFS 通过 `open_tree(OPEN_TREE_CLONE|AT_RECURSIVE)` + `move_mount` 创建目标，Magic Mount 通过 `MS_BIND` 克隆，克隆会继承源挂载的传播类型，因此 Magic 目标可能带着源挂载的 peer group 进入传播表，与不带 `shared:` 的 OverlayFS 目标在同一方案内混排，并从内核的全局组号分配器额外取号。为此 Magic Mount 在每个 bind 之后立即对该目标执行 `MS_PRIVATE`，目录 mount-move 之后再对整棵子树执行 `MS_PRIVATE|MS_REC`：`MS_PRIVATE` 只影响本挂载，源的 peer group 不受影响。归一化是尽力而为，失败只告警；挂载阶段结束时会读回 mountinfo 核对活动目标是否仍在 peer group 内，并在日志中给出 `propagation=private|shared:N|slave:N`，避免传播表变化无人察觉。`src/sys/mountinfo.rs` 因此除 `mnt_id` 外同时解析 `shared:` 与 `master:`。

`status` 另外暴露 VFS 字段：`vfs_modules` 列出本次启动使用 VFS 的模块，`vfs_active_mounts` 记录注入成功的目标路径，`vfs_provider` 为本次启动唯一绑定的内核 Provider（只有 `hm`，即 HM 自有的 `hybridmount` 模块）。这些字段与配置一并由启动流水线与状态层写入启动快照。

`storage_mode` 记录本次启动实际创建的 overlay staging 后端，取值为 `tmpfs`、`ext4` 或哨兵值 `none`。只使用 VFS 或 Magic Mount 的启动不会创建 staging，此时写入 `none`：动态模块描述与 WebUI 都据此显示实际运行的挂载后端（例如 VFS），不会假报 Tmpfs 或 Ext4。

## 共享节点树契约

scanner 对每个模块源节点只读记录类型、源路径和 `.replace` 标记；planner 将其映射到真实目标路径并标注 `overlay`、`magic`、`vfs` 或 `ignore`。同一目标可以保留多个同后端模块贡献，用于 OverlayFS lowerdir 优先级；跨后端的普通目录可作为共享结构节点，文件、类型或 `.replace` 冲突仍在规划阶段报错。

OverlayFS staging 只物化树中标注为 `overlay` 的节点，因此同模块内的 magic/ignore 子树不会被整目录复制进 lowerdir，Overlay 目录可以安全包含后续由 Magic 处理的子路径。目录 `.replace` 转换为 `trusted.overlay.opaque=y`，whiteout 保留为设备节点，符号链接不跟随。Magic Mount 在 OverlayFS 完成后遍历同一棵树的 `magic` 分支；未被选中但承载选中后代的目录只作为结构父链，不会改变后端归属。由于执行顺序固定为 OverlayFS → Magic Mount，Magic `.replace` 目录若包含 Overlay 后代会在规划阶段报冲突，避免后执行的目录替换遮住先前挂载。

VFS 目标不得存在被 Overlay/Magic 挂载的祖先目录（反之亦然），否则 plan 阶段报
`PlanConflict`。VFS 不是真实挂载，因此不参与 KSU try-umount 列表；其成功目标记录在
`vfs_active_mounts`，并计入 `active_mounts` 供 WebUI 展示活动挂载点。

VFS 规则以虚拟路径为键，同一目标只下发一条（`node.sources` 中模块顺序靠后者获胜）。
回滚是定向删除：下发前登记本次完整批次，失败时对其逐条 `DEL_RULE` 并容忍 `ENOENT`，
不使用 `CLEAR_RULES`，因此不会改动 `hybridmount` 中其它调用方预先安装的规则。

VFS 的 `.replace` 目录会生成 opaque 规则，并在 planner 阶段独占整个子树；
若子树中混入 OverlayFS 或 Magic Mount 目标，立即返回 `PlanConflict`。

VFS Provider 只有 HM 自有的 `hybridmount`。启动时先探测内建的 key type `hybridmount`；未响应则从
`vfs/binaries/` 加载与内核线及 Android/GKI 标签精确匹配的预编译模块并重新探测。若仍无可用
Provider、版本不兼容或检测到外来 NoMount，则按 `vfs_strict` 选择失败或降级跳过。

加载必须发生在规划**之前**：planner 在 `vfs_available` 为 false 时会把所有 `vfs` 规则
（模块默认与路径规则）解析为 `ignore`，执行器随即看到空的 vfs 模块集合而提前返回，走不到
自己的加载分支。若等到执行阶段才加载，这个先有鸡还是先有蛋的循环会让内核未内建
`hybridmount` 的设备——也就是随附预编译模块唯一服务的对象——永远无法加载该模块。因此
`vfs::ensure_loaded_for_plan` 先探测、必要时加载，再以加载后的探测结果作为
`PlanInput.vfs_available`。

该探测结果同时喂给所有对外展示面：为 false 时规划阶段即降级为 `ignore`，避免配置、状态计数
或 WebUI 页面向用户展示一个本机执行不了的后端。WebUI 的 VFS 选项、计数与说明行也都读取同
一个探测结果。

`vfs_strict` 的失败判定同样在规划前完成，否则上述降级会让它失去作用。只有配置中确有规则选择
`vfs` 时该选项才生效：配置为 overlay 或 magic 的设备即使打开它也能正常启动。

## 验证边界

主机侧可运行 Rust 单元测试、Clippy、WebUI 测试/类型检查和生产构建。Android 三架构编译由 `cargo xtask build` 或 CI 完成。真实 mount、loop、SELinux 与 KernelSU/APatch 交互必须在受支持设备上验证。

运行期新增 `boot` 和 `runtime status|load ID|unload ID|reload ID|prepare-reboot`，命令、升级与恢复限制见 [RUNTIME.md](RUNTIME.md)。`scan.ret` 和 `run/state.json` 在成功热操作后同步更新，不再仅代表启动瞬间。
