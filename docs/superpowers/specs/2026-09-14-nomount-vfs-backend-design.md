# Hybrid Mount 接入 NoMount VFS 后端设计

- 状态：已批准设计，待实现
- 日期：2026-09-14
- 目标仓库：`Hybrid-Mount/meta-hybrid_mount`（分支 `dev`）
- 参考上游：`maxsteeel/nomount`（默认分支 `master`，协议版本 `NOMOUNT_VERSION "20"`，License GPL-3.0）
- 归属：本设计及派生实现需标注上游作者 `maxsteeel`（见第 14 节）

---

## 1. 背景

NoMount 是一个运行在 Linux VFS 层的内核路径重定向框架。它不创建任何 mount，
而是按需劫持真实目录的 `inode->i_op` / `inode->i_fop` 与 `super_block->s_op`，
在 `lookup` 与目录迭代中动态注入、隐藏条目，并把文件读写转发到真实文件；
用户态通过 keyring 的 `add_key` 单页协议下发规则。

Hybrid Mount（下称 HM）是 KernelSU / APatch 的混合挂载元模块，现有
OverlayFS 与 Magic Mount 两个执行后端，启动流水线为：

```text
config → scan → plan → storage → overlay → magic → commit
```

本设计把 NoMount 的 VFS 重定向能力引入 HM，作为第三个后端 `vfs`。核心约束：

1. **兼容 NoMount 内核协议**：设备已集成 NoMount（内核 built-in 或官方 LKM）时，
   HM 应能直接驱动它；
2. **必须拥有独立内核实现**：HM 不能依赖设备预装 NoMount，需自带可独立分发、
   独立演进的 VFS 内核实现；
3. **两者二选一**：HM 自有内核实现与 NoMount 内核实现在任何时刻互斥，无论内核
   集成层还是元模块调用层都只允许一个处于活动状态。

---

## 2. 目标与非目标

### 2.1 目标

- 在 `config.toml` 与规划器中新增 `vfs` 后端，沿用“路径规则 > 模块 default_mode > 全局 default_mode”。
- 复用现有共享节点树（`MountTree`），新增“树 → VFS 规则”的纯函数映射。
- 用户态直接实现 NoMount wire 协议（keyring + `nm_payload`），同时驱动 K1 / K2。
- 提供 HM 自有的 VFS 内核实现（fork NoMount，独立 GPL-3.0-only 子树）。
- Provider 选择与互斥作为一等不变量，进入规划/执行/状态/可观测性。
- 无内核支持时安全降级到 overlay / magic，不破坏现有行为。

### 2.2 非目标

- 不修改、fork 后不影响 NoMount 上游仓库；
- 不与 NoMount 元模块并列安装（KernelSU 同一时刻只允许一个元模块）；
- 不重写 HM 现有 OverlayFS / Magic Mount 语义；
- 不把 VFS 后端伪装成真实挂载（不污染 `/proc/mounts`）。

---

## 3. 术语

| 术语 | 含义 |
| --- | --- |
| K1 | 设备原生 NoMount 内核实现（built-in 或官方 NoMount LKM） |
| K2 | HM 自有的 VFS 内核实现（fork NoMount，独立构建与分发） |
| Provider | 当前被 HM 绑定并驱动的内核实现，取值 `nomount` / `hm` |
| wire 协议 | `add_key("nomount", "trigger", &payload)` + `nm_payload` 布局与命令集 |
| 遮蔽 | 祖先目录被其他后端挂载后，其后代路径的 VFS 规则不可见 |

---

## 4. 现状与调研结论

### 4.1 NoMount 内核机制（`kernel/src/nomount.c`）

- 添加规则时用 `kern_path()` 定位虚拟路径的真实父目录，然后：
  - `inode->i_op` → `nm_iop.fake_iop`（`lookup = nomount_hijacked_lookup`）；
  - `inode->i_fop` → `nm_fop.fake_fop`（`iterate_shared/iterate = nomount_hijacked_iterate_dir`）；
  - `sb->s_op` → `nm_sop.fake_sop`（`destroy_inode/drop_inode/evict_inode`），并代理 `s_xattr`（`nomount_hijack_superblock`）。
- 只在有规则的目录上按需劫持，并非全局 hook。
- 注入文件的操作向量（read/write/mmap/ioctl/splice/fsync/getattr/setattr/xattr）转发到 `rule->r_path`；真实 inode 被置 `S_PRIVATE`。
- whiteout：规则无真实路径，`d_revalidate` 返回 `!inode` 使条目“消失”。
- 虚拟目录拓扑：注入 `/system/etc/foo` 时若父规则不存在，自动创建 `NM_FLAG_VIRTUAL_DIR` 占位（`nomount_generate_virtual_topology`），删除时剪枝。
- UID 隔离：`nomount_uids` + `nomount_is_uid_blocked(current_fsuid())`，命中直接走原生路径。
- 索引：ART（`nomount_art_root`）；目录子项：有序哈希数组 + bloom mask + seqcount/RCU。

### 4.2 NoMount 控制协议

传输：

```c
struct nm_workspace {            /* 4096 对齐，payload 在偏移 0 */
    struct nm_payload payload;   /* 4096 字节 = 一页 */
    char cwd[PATH_MAX];
};
payload->status = -1;
unsigned long ptr = (unsigned long)payload;
sys5(SYS_ADD_KEY, "nomount", "trigger", &ptr, sizeof(ptr), -1);
/* 内核回写 payload->status，并通过 buffer/arg1/data_size 返回列表数据 */
```

内核 `nm_key_preparse` 校验 `capable(CAP_SYS_ADMIN)`，取 8 字节（64 位）/ 4 字节（32 位）
用户指针，`get_user_pages_fast` 锁页 → `kmap` → 校验 magic → 处理 → `set_page_dirty_lock`，
返回 `-ECANCELED`，因此 key 不会真正创建或残留。

Payload：

```c
struct nm_payload {              /* 共 4096 B, packed */
    u64 magic;                   /* 0x4E4F4D4F554E54 "NOMOUNT" */
    u32 cmd;                     /* NM_CMD_* */
    u32 target_uid;              /* ADD/DEL_UID */
    int status;                  /* 内核回写 */
    u32 arg1;                    /* 批处理/分页游标 */
    u32 data_size;               /* buffer 有效长度 */
    char buffer[4068];
};
struct nm_rule_hdr { u32 flags; u32 uid; u16 v_len; u16 r_len; } /* 12 B */
struct nm_del_hdr  { u32 uid; u16 v_len; }                       /* 6 B */
```

命令（`NM_CMD_*`）：

```text
UNSPEC=0  GET_VERSION=1  ADD_RULE=2  DEL_RULE=3
ADD_UID=4 DEL_UID=5 CLEAR_ALL=6 CLEAR_RULES=7 CLEAR_UIDS=8
GET_LIST=9 GET_UIDS=10
```

标志：`IS_DIR=1`（内核置）、`VIRTUAL_DIR=2`（内核置）、`WHITEOUT=4`（用户态置）。
whiteout 时 `r_len` 被内核强制为 0。

批处理与分页：ADD_RULE / DEL_RULE 的 buffer 是连续记录流，`arg1` 回写已消费偏移；
GET_LIST / GET_UIDS 以 `arg1` 为起始索引分页直到 `data_size == 0`。
内核侧 `v_len/r_len < PATH_MAX`。

### 4.3 NoMount 元模块扫描语义（`module/metamount.sh`，可复用）

- 遍历 `/data/adb/modules/*`，跳过 `disable/remove/skip_mount` 与自身；
- 对受管分区：
  - 带 `trusted.overlay.opaque="y"` 的目录与 `.replace` → `nm rule add --whiteout`；
  - 普通文件 / 符号链接 → `nm rule add <vpath> <rpath>`；
  - 目录本身不注入，目录合并依赖内核虚拟拓扑；
- 分区提升：`/system/odm/...` → `/odm/...`；
- 加载 LKM 时使用 `/data/adb/nomount/.booting` 信号量做 bootloop 熔断，
  `boot-completed.sh` 清除。

### 4.4 HM 现状（接入面）

- `src/config.rs`：`enum Mode { Overlay, Magic, Ignore }`，`#[serde(rename_all = "lowercase")]`，`deny_unknown_fields`。
- `src/mount_tree.rs`：共享 `MountNode/MountSource`；`NodeFileType` 已含 `RegularFile/Directory/Symlink/Whiteout`；`structural_sources` 承载结构父链。
- `src/plan/mod.rs`：`MountPlan { tree, overlay_ops, overlay_files, overlay_module_ids, magic_module_ids }`；`register()` 做跨后端冲突检测；`ensure_replace_backend_consistency()` 只校验 overlay↔magic。
- `src/pipeline.rs`：overlay → magic → KSU try-umount → 状态快照，失败事务式回滚。
- `src/state.rs`：`RunState { overlay_modules, magic_modules, active_mounts, overlay_active_mounts, magic_active_mounts, mode_stats }`。
- 全仓库当前无任何 `nomount` / `vfs` 引用。

---

## 5. 架构总览

```text
                    config.toml / WebUI
                           │  Mode::Vfs
                           ▼
   scan ──► MountTree ──► plan ──► MountPlan { vfs_module_ids, ... }
                           │
                           ▼
              pipeline 执行阶段（overlay → magic → vfs）
                           │
                           ▼
                 src/vfs/ 后端（纯函数 + 协议适配）
                           │  握手 GET_VERSION
                           ▼
              ┌────────────┴────────────┐
              ▼                         ▼
        K1: NoMount 内核           K2: HM 自有 VFS 内核
        (built-in / 官方 LKM)      (fork, HM 独立分发)
              └────────────┬────────────┘
                           ▼
                同一 wire 协议：add_key("nomount")
                           │
              二者互斥：同一时刻只有一个 Provider 活动
```

要点：

- **wire 层统一**：K1 与 K2 注册同名 `nomount` key type、使用相同 payload/命令/flags，
  因此 HM 用户态只需一套协议实现，甚至 NoMount 官方 `nm` CLI 也能驱动 K2。
- **互斥层**：由 `src/vfs/backend.rs` 在启动时探测并绑定唯一 Provider，详见第 7 节。

---

## 6. Wire 协议契约

### 6.1 HM 侧实现方式

HM 用户态在 Rust 内直接实现 `add_key` wire 协议（而非只调用外部 `nm` CLI），原因：

- 需要同时驱动 K1 与 K2，统一协议可避免依赖设备是否安装 `nm`；
- 可对 payload/hdr 做字节级单测与批量下发。

```rust
// 设计示意（非最终实现）
const MAGIC: u64 = 0x4E4F_4D4F_554E_54; // "NOMOUNT"
const PAYLOAD_LEN: usize = 4096;

struct Payload { /* magic, cmd, target_uid, status, arg1, data_size, buffer[4068] */ }
```

注意：keyring 要求 payload 所在页可写、且 `offset_in_page(ptr) + 4096 <= PAGE_SIZE`；
实现须按页对齐分配（可复用 HM 现有临时内存策略），不得跨越页边界。

### 6.2 版本握手

- 启动先发 `GET_VERSION`；解析返回字符串（当前为 `"20"`）。
- HM 维护“支持的协议版本集合”，初始为 `{20}`。
- 版本不受支持 → 拒绝该 Provider，进入第 7 节选择流程；不尝试“猜测”兼容。
- K2 由 HM 控制版本，声明其兼容下限；K1 需落在支持集合内。

### 6.3 路径语义

- 下发的 virtual / real 路径一律为绝对路径（HM 直接构造，不经 shell cwd）。
- 保留 `PATH_MAX` 限制；单条记录 `sizeof(hdr) + v_len + r_len <= 4068`。
- 分区提升沿用 planner 的 `map_target` 结果。
- 符号链接源：上游内核 `kern_path(..., LOOKUP_FOLLOW, ...)` 会跟随真实路径，
  该语义需实机确认（见第 17 节）。

---

## 7. Provider 选择与互斥（硬不变量）

> **同一时刻只允许一个 VFS 内核实现处于活动状态；K1 与 K2 绝不并存。**
> 该约束同时适用于内核集成层（编译期）与元模块调用层（运行期）。

### 7.1 内核集成层（编译期）

- HM 的 `module/vfs/setup.sh` 在 patch 内核前探测：
  - 已存在 `CONFIG_NOMOUNT=y/m`、`fs/nomount/` 源码，或本脚本自身产生的集成标记；
  - 运行期以 `/proc/modules` 是否已有 `nomount` 模块作为 LKM 冲突判据；
  - 命中则**拒绝集成并提示**，要求用户二选一。
- 反向：文档明确要求同一内核树只集成一套，避免 `register_key_type("nomount")` 二次注册失败。
- K2 集成后，内核即成为 HM 的 Provider（built-in 路径），无需加载 LKM。

### 7.2 元模块调用层（运行期）

选择顺序（`src/vfs/backend.rs`）：

1. **探测已有 Provider**：发送 `GET_VERSION`；可响应且版本受支持 → 直接绑定该 Provider，
   **不再加载任何内核模块**。此时若 HM 本次启动的运行时标记表明 K2 已由 HM 加载，
   则记为 `vfs_provider = hm`，否则记为 `nomount`（区分方式见第 17 节未决点 6）。
2. **K1 不存在**：若存在与本机 kernel line / Android / arch **精确匹配**的 K2 LKM →
   加载并绑定，`vfs_provider = hm`；加载失败必须 `rmmod` 回滚，不得留下半初始化模块。
3. **均不可用**：`Mode::Vfs` 不可执行 → 按配置降级到 overlay/magic，并在状态中标记
   `vfs_unavailable`，或按“严格模式”直接失败（见第 12 节）。

运行期规则：

- 绑定结果**本次启动内固定**，禁止 K1↔K2 热切换；切换需清空规则并冷启动。
- 若运行中发现两者同时可见（同名 key type 冲突 / 二次注册错误）→ 判为
  `Error::VfsProviderConflict`，**不下发任何规则**。
- 清理只针对绑定的 Provider，绝不 `rmmod` 对方模块。
- 加载 K2 前必须复用 HM 现有的 boot 熔断机制（写标记 → 成功清除 → 崩溃后跳过），
  避免 LKM 引入新的启动循环。

### 7.3 状态与可观测性

- `RunState.vfs_provider: Option<VfsProvider>`，取值 `"nomount" | "hm"`。
- `status` 与 WebUI 展示当前 Provider、探测结果与互斥判定。
- 失败快照保留 Provider 字段，便于定位“加载了错误的实现”类问题。

---

## 8. 数据模型变更

### 8.1 `src/config.rs`

- `enum Mode` 增加 `Vfs`，`as_str() -> "vfs"`，serde 小写。
- 顶部文档示例更新 `default_mode = "overlay" # overlay | magic | vfs`。
- `default_mode = "ignore"` 的废弃校验不变。
- 新增可选严格开关 `vfs_strict`：为 `true` 时 VFS 不可用即启动失败，`false` 时降级
  （默认值见第 17 节未决点 4）。

### 8.2 `src/plan/mod.rs`

- `MountPlan` 增加 `vfs_module_ids: Vec<ModuleId>`。
- 新增 `collect_vfs()`（对齐 `collect_magic()`）。
- `register()` 将 `Vfs` 纳入同目标冲突判定。

### 8.3 `src/state.rs`

- `RunState` 增加 `vfs_modules: Vec<String>`、`vfs_active_mounts: Vec<String>`、
  `vfs_provider: Option<String>`。
- `ModeStats` 增加 `vfs: usize`。
- 语义明确：`active_mounts` 只统计真实挂载（overlay + magic）；VFS 是注入，不计入，
  避免污染 `/proc/mounts` 语义与 KSU try-umount 列表。

---

## 9. 规则映射（`src/vfs/rules.rs`，纯函数）

输入：`MountTree` 中标注为 `Vfs` 的来源；输出：`Vec<VfsRule>`。

| 节点类型 | 映射 |
| --- | --- |
| `RegularFile` | injection：`vpath -> rpath` |
| `Symlink` | injection（语义待第 17 节确认） |
| `Whiteout` | `--whiteout vpath` |
| `Directory` | 只作为遍历结构，不产生规则；虚拟拓扑由内核补 |
| `.replace` 目录 | 先 whiteout 目录，再注入其下条目 |

- 目标路径取 planner 的 `map_target` 结果（含分区提升）。
- 一个目标的多模块层：VFS 无 lowerdir 叠加概念，同一真实路径只能有一个后端；
  同后端多模块命中同一目标时，按模块顺序取最后生效者并记录覆盖，策略需与现有
  `MountNode.sources` 排序对齐（见第 17 节未决点 5）。
- 目录结构父链（`structural_sources`）不产生 whiteout 或注入。

---

## 10. 规划不变量与冲突

沿用现有不变量：

- 同一文件路径只能进入一个后端；
- 普通目录可被多个后端作为结构节点共享；
- 文件 / 类型 / `.replace` 冲突在 plan 阶段显式报错，executor 不裁决。

VFS 新增：

1. **同目标唯一**：`Vfs` 与 overlay/magic 命同一目标 → `Error::PlanConflict`。
2. **遮蔽约束（新增，关键）**：
   - VFS 目标不得存在被 Overlay / Magic 挂载的祖先目录；
   - 被 Overlay / Magic 挂载的目录不得包含 VFS 后代；
   - 违反 → `Error::PlanConflict`（附加 `shadowed_by` 说明）。
   原因：VFS 规则作用在真实目录上，任何祖先挂载点都会遮蔽其后代注入。
3. **`.replace` 一致性**：把 `Vfs` 纳入 `ensure_replace_backend_consistency`。
   - Vfs `.replace` 目录若包含 Overlay 后代 → 冲突（Vfs 替换先于/晚于 overlay 都不安全）；
   - Magic `.replace` 与 Vfs 后代的组合同样报错。

---

## 11. 执行顺序与回滚（`src/pipeline.rs`）

- 阶段顺序：`overlay → magic → vfs`。VFS 最后执行，因为它是“非挂载”的最终注入；
  遮蔽冲突已在 plan 阶段排除，顺序不再需要动态裁决。
- `needs_runtime_temp` 需计入 `vfs_module_ids`（若协议实现需要临时页内存）。
- 回滚：每个成功操作注册进现有 `Transaction`：
  - 单条：`DEL_RULE`；
  - 整体：`CLEAR_RULES`；
  - 启动失败时清理本次下发的规则，不触碰 Provider 的其它状态（如 UID 隔离可独立配置）。
- Boot 安全：K2 加载与规则下发前写 `/data/adb/hybrid-mount/vfs_boot_guard`，
  成功后删除；遗留标记表示上次启动失败，下次跳过 VFS 后端并保留失败快照。

---

## 12. 配置 / CLI / WebUI 契约

- `config.toml`：`default_mode` 与 `[rules.*.paths]` 接受 `"vfs"`；未知字段仍拒绝。
- `show-config` / `save-config`：自动支持新枚举值，无需新命令。
- `status`：新增 `vfs_modules`、`vfs_active_mounts`、`vfs_provider`、`vfs_unavailable`（可选）。
- `mode_stats`：新增 `vfs`。
- WebUI：模式选择项增加 “VFS”；模块/路径规则可使用；状态页展示 Provider 与 VFS 注入计数。
- 兼容：旧配置不含 `vfs`，行为不变；新字段均可选，升级不破坏。

---

## 13. 独立内核实现子系统（K2）

- **来源**：fork NoMount 内核源码（已批准），在其上独立演进与分发。
- **位置**：`module/vfs/src/`（`fs/nomount/` 等），License **GPL-3.0-only**，
  与现有 `module/lkm/`（GPL-2.0-only，ext4 nuke）分属两个许可子树，禁止合并。
- **ABI**：注册同名 `nomount` key type，payload / 命令 / flags 与 NoMount 兼容，
  使 HM 用户态与 NoMount `nm` CLI 均可驱动。
- **分发形态**：
  - built-in：提供 `module/vfs/setup.sh` 自动 patch（含 7.1 的互斥探测）；
  - LKM：`module/vfs/binaries/nomount-<gki>-<kernel>.ko` + `list.txt` SHA256 校验，
    复用 HM 现有的“精确候选 + 校验 + boot 熔断”机制。
- **构建**：`xtask` 增加内核模块构建矩阵（kernel line × Android/GKI × arch），
  产物进入发布 ZIP；具体覆盖范围见第 17 节未决点。
- **上游同步**：记录所 fork 的 NoMount commit，便于后续 rebase 与差异追踪。

---

## 14. 归属 / co-author

- 派生文件头保留 NoMount 版权与 `SPDX-License-Identifier: GPL-3.0-only`，并注明来源。
- 集成提交加 `Co-authored-by: maxsteeel <109047395+maxsteeel@users.noreply.github.com>` trailer。
- 内核模块保留 `MODULE_AUTHOR("maxsteeel")`，必要时追加 HM 维护者。
- `module.prop` 的 `author` 与 README 的 Special Thanks 收录 NoMount / maxsteeel。
- 建议向 NoMount 提 issue/RFC，就 `nm` ABI 版本兼容窗口达成一致，降低上游改版导致失配的风险。
- 许可兼容性：NoMount GPL-3.0 与 HM 核心 GPL-3.0-only 兼容；不得并入 GPL-2.0-only 的
  `module/lkm/` 子树。

---

## 15. 测试与验证

### 15.1 主机侧（无需设备）

- `tree → VfsRule` 纯函数单测：对照 `metamount.sh` 语义，覆盖文件 / 符号链接 /
  whiteout / 目录结构 / `.replace` / 分区提升。
- 协议编解码单测：`nm_payload`、`nm_rule_hdr`、`nm_del_hdr` 的字节级布局与批处理游标。
- Provider 选择逻辑：以可注入 mock 覆盖 K1 命中 / K1 缺失加载 K2 / 双重可见冲突 / 加载失败回滚。
- 冲突与遮蔽单测：同目标多后端、祖先挂载遮蔽、`.replace` 组合。
- 现有门禁：`cargo fmt`、`clippy -D warnings`、`cargo test --workspace`、禁用符号检查。

### 15.2 内核侧

- 各 kernel line 的 K2 编译通过；
- 实机 ABI 校验：key type 注册、`GET_VERSION`、注入 / whiteout / UID 隔离。

### 15.3 实机矩阵

| 场景 | 验证点 |
| --- | --- |
| K1（原生 NoMount built-in） | 握手、注入、whiteout、`.replace`、UID、`/proc/mounts` 干净 |
| K1（官方 NoMount LKM） | 同上，且 HM 不重复 `insmod` |
| K2（HM 自有 LKM） | 同上，且 NoMount `nm` CLI 可驱动 |
| 互斥 | 先装 NoMount 再装 HM、以及已加载 K2 时注入 K1 痕迹 → 拒绝且不执行规则 |
| 降级 | 无任何内核支持时按配置回落 overlay/magic，状态正确 |
| 遮蔽冲突 | VFS 祖先被挂载用例在 plan 阶段报错 |

---

## 16. 风险

| 风险 | 缓解 |
| --- | --- |
| 内核 ABI 不稳定 | 版本握手 + 支持集合；K2 由 HM 自控版本 |
| 覆盖设备少 | GKI 5.10+ 免改内核；旧内核走 built-in patch |
| whiteout 目录 + 子文件语义 | 实机优先验证（第 17 节） |
| 遮蔽顺序 | plan 阶段强制不变量 |
| 状态语义混淆 | VFS 与挂载分字段统计 |
| K2 引入启动循环 | 复用 HM boot 熔断 + 严格回滚 |
| `S_PRIVATE` / xattr 代理副作用 | 实机观察，必要时在上游反馈 |
| 上游改版导致失配 | 记录 fork commit + 协商 ABI 窗口 |

---

## 17. 未决问题

1. **whiteout 目录后再注入其子文件**：上游 `nomount_generate_virtual_topology` 在父规则
   非 `IS_DIR` 时返回 `-ENOTDIR`，而 whiteout 规则不带 `IS_DIR`，需实机确认
   `.replace` 目录的实际行为，并据此决定映射策略（目录 whiteout vs 逐子项 whiteout）。
2. **符号链接源**：上游对真实路径使用 `LOOKUP_FOLLOW`，注入符号链接是否保留链接语义需确认。
3. **K2 覆盖矩阵**：是否对齐 NoMount 的 5.4–6.16 全量，还是先覆盖 GKI 5.10+ 主流版本。
4. **严格模式默认值**：`vfs_strict` 默认 false（降级）还是 true（失败）需产品决策。
5. **同目标多模块 VFS 规则**：最终生效者选择策略需与现有 sources 排序对齐并在文档固化。
6. **K1 与 K2 的 wire 层区分**：两者注册同名 key type，探测只能判断“有 Provider 活动”，
   无法直接区分实现来源。候选方案：K2 在 `GET_VERSION` 返回串附带实现标识（需保证
   `nm` CLI 仍可读），或 K2 额外注册独立探测 key type；确定前 `vfs_provider` 以 HM
   运行时标记判定。

---

## 18. 分期计划（本文档不包含实现）

- **Phase 0**：冻结 wire 协议与版本集合；记录 fork commit；确认第 17 节第 1–3 项。
- **Phase 1**：`src/vfs/rules.rs` 纯函数 + 单测（不接触设备）。
- **Phase 2**：`src/vfs/protocol.rs` 字节级协议 + `backend.rs` Provider 选择与互斥（mock 测试）。
- **Phase 3**：K2 内核 fork 子树 + `setup.sh` + LKM 构建矩阵 + boot 熔断。
- **Phase 4**：pipeline 阶段接入、回滚、状态字段、`status`/WebUI、打包与文档。

---

## 19. 参考

- NoMount：https://github.com/maxsteeel/nomount
- NoMount 内核集成：`kernel/README.md`（built-in / LKM / setup.sh）
- HM 架构契约：`docs/ARCHITECTURE.md`
- HM 挂载树：`src/mount_tree.rs`；规划器：`src/plan/mod.rs`；流水线：`src/pipeline.rs`
