# Hybrid Mount 自有 VFS 后端设计（K2 单内核）

- 状态：设计修订 v2（已移除 K1 支持），待实现
- 日期：2026-09-14（v2）
- 目标仓库：**Hybrid-Mount/meta-hybrid_mount**
- 上游参考：**maxsteeel/nomount**（协议版本 "20"，License GPL-3.0）
- 归属：fork 与派生实现需标注上游作者 maxsteeel（见第 14 节）

---

## 0. 修订记录（决策）

| 版本 | 决策 | 状态 |
| --- | --- | --- |
| v1 | HM 同时兼容 K1（NoMount 原生内核）与 K2（HM 自有内核），两者硬互斥 | 已废弃 |
| **v2** | **放弃 K1；HM 只驱动 HM 自有内核实现（K2）**，ABI / CLI / 协议由 HM 自控 | **本版采用** |

v2 决策清单：

- **D1 放弃 K1**：HM Rust 元模块不再探测或驱动 NoMount 原生内核；Provider 只有 HM 一种。
- **D2 ABI 自有**：K2 使用 HM 专属 key type（工作名 hybridmount），协议以 NoMount v20 的 payload 布局为基线，但 HM 可自由增量扩展 flag / 命令 / 语义。
- **D3 .replace = 显式 opaque**：K2 新增用户态可置的 opaque 目录语义；v1 计划中依赖 NoMount 内部 VIRTUAL_DIR 标志的 userspace hack 作废。
- **D4 独有 CLI**：HM 二进制新增 vfs 子命令族，作为 K2 的官方控制面。
- **D5 归属保留**：K2 仍是 NoMount 源码的 fork，GPL-3.0-only、版权与 Co-authored-by 保留。
- **D6 单向守卫**：不再做双向 Provider 互斥，只保留“发现外来 NoMount → 拒绝加载 K2”的安全检查，避免两套实现同时劫持 inode 操作向量。

放弃 K1 的理由：K1 的 .replace 已被证实于内核层面损坏（第 4.3 节），保留它只能得到一条更差、且需长期维护的路径；而 K2 无论如何都必须自建 LKM 矩阵。代价是放弃“复用 NoMount 官方预编译 LKM”这一低门槛覆盖捷径。

---

## 1. 背景

NoMount 是运行在 Linux VFS 层的路径重定向框架：不创建 mount，而是按需劫持真实目录的 inode->i_op / inode->i_fop 与 super_block->s_op，在 lookup 与目录迭代中动态注入、隐藏条目，并把文件读写转发到真实文件；用户态通过 keyring 的 add_key 单页协议下发规则。

Hybrid Mount（下称 HM）是 KernelSU / APatch 的混合挂载元模块，现有 OverlayFS 与 Magic Mount 两个执行后端，启动流水线为：

~~~
config → scan → plan → storage → overlay → magic → commit
~~~

本设计为 HM 引入第三个后端 vfs，并采用**自有内核实现**（K2）：fork NoMount 内核源码，在其上独立演进与分发。核心约束：

1. **自有实现**：HM 必须拥有可独立分发、独立演进的 VFS 内核实现，不依赖设备预装 NoMount；
2. **自有 ABI**：K2 的 key type / 协议版本 / 命令集由 HM 定义，不受上游兼容窗口约束；
3. **单向兼容**：HM 元模块驱动 K2；K2 不接受 NoMount 元模块（nm CLI）驱动；
4. **不并存**：同一内核中 K2 与 NoMount 原生实现不得同时活动（双重劫持）。

---

## 2. 目标与非目标

### 2.1 目标

- 在 config.toml 与规划器中新增 vfs 后端，沿用“路径规则 > 模块 default_mode > 全局 default_mode”。
- 复用共享节点树（MountTree），新增“树 → VFS 规则”的纯函数映射。
- HM 用户态直接实现 K2 wire 协议（keyring + 单页 payload），只驱动 K2。
- 提供 HM 自有的 VFS 内核实现（fork NoMount，独立 GPL-3.0-only 子树）。
- 提供 K2 专属 CLI（hybrid-mount vfs ...），作为官方控制与诊断面。
- .replace 以显式 opaque 目录实现，语义与 OverlayFS 的 trusted.overlay.opaque 对齐。
- 无内核支持时安全降级到 overlay / magic，不破坏现有行为。

### 2.2 非目标

- **不驱动、不探测 NoMount 原生内核（K1）**；设备若已集成 NoMount，HM 的 VFS 后端不可用，按配置降级到 overlay / magic。
- 不与 NoMount 元模块互操作（K2 的 key type 与协议与 nm CLI 不同）。
- 不修改、不影响 NoMount 上游仓库。
- 不把 VFS 后端伪装成真实挂载（不污染 /proc/mounts）。
- 不实现 K1/K2 双向互斥；只保留单向安全守卫（第 7 节）。

---

## 3. 术语

| 术语 | 含义 |
| --- | --- |
| K2 | HM 自有的 VFS 内核实现（fork NoMount，独立 ABI 与版本） |
| Provider | 当前被 HM 绑定并驱动的内核实现；v2 只有取值 hm |
| wire 协议 | add_key(HM_KEY_TYPE, "trigger", &payload) + K2 payload 布局与命令集 |
| 兼容矩阵 | HM 元模块 × K2 内核 = 支持；K1 与 NoMount 元模块均不在支持范围 |
| opaque 目录 | 目标目录保持可见，真实条目全部隐藏，仅显示注入子项（即 .replace 语义） |
| 遮蔽 | 祖先目录被其他后端挂载后，其后代路径的 VFS 规则不可见 |

---

## 4. 现状与调研结论

### 4.1 上游内核机制（fork 基线，kernel/src/nomount.c）

- 添加规则时用 kern_path() 定位虚拟路径的真实父目录，然后：
  - inode->i_op → nm_iop.fake_iop（lookup = nomount_hijacked_lookup）；
  - inode->i_fop → nm_fop.fake_fop（iterate = nomount_hijacked_iterate_dir）；
  - sb->s_op → nm_sop.fake_sop，并代理 s_xattr（nomount_hijack_superblock）。
- 只在有规则的目录上按需劫持，并非全局 hook。
- 注入文件的操作向量（read/write/mmap/ioctl/splice/fsync/getattr/setattr/xattr）转发到 rule->r_path；真实 inode 被置 S_PRIVATE。
- whiteout：规则无真实路径，d_revalidate 返回 !inode 使条目“消失”。
- 虚拟目录拓扑：注入 /system/etc/foo 时若父规则不存在，自动创建 NM_FLAG_VIRTUAL_DIR 占位（nomount_generate_virtual_topology），删除时剪枝。
- 虚拟目录语义：nm_dir_lookup 对非注入名返回负 dentry；nm_dir_iterate_dir 只发出注入子项。这正是 opaque 目录所需的语义。
- 索引：ART（nomount_art_root）；目录子项：有序哈希数组 + bloom mask + seqcount/RCU。
- UID 隔离：nomount_is_uid_blocked(current_fsuid())，为 O(n) 线性扫描（第 13.3 节优化项）。

### 4.2 上游控制协议（K2 协议基线）

传输示例（上游 key type 为 nomount；K2 改为 HM 专属名）：

~~~c
struct nm_workspace {            /* 4096 对齐，payload 在偏移 0 */
    struct nm_payload payload;   /* 4096 字节 = 一页 */
    char cwd[PATH_MAX];
};
payload->status = -1;
unsigned long ptr = (unsigned long)payload;
sys5(SYS_ADD_KEY, "nomount", "trigger", &ptr, sizeof(ptr), -1);
/* 内核回写 payload->status，并通过 buffer/arg1/data_size 返回列表数据 */
~~~

内核 nm_key_preparse 校验 capable(CAP_SYS_ADMIN)，取用户指针，get_user_pages_fast 锁页 → kmap → 校验 magic → 处理 → 返回 -ECANCELED，因此 key 不会真正创建或残留。

Payload：

~~~c
struct nm_payload {              /* 共 4096 B, packed */
    u64 magic;                   /* 0x4859425249444D4F "HYBRIDMO"（已换为 HM 专属值，见 17.2） */
    u32 cmd;                     /* K2_CMD_* */
    u32 target_uid;              /* ADD/DEL_UID */
    int status;                  /* 内核回写；v2 语义见第 6.3 节 */
    u32 arg1;                    /* 成功：已消费偏移；失败：失败记录下标（D2 扩展） */
    u32 data_size;               /* buffer 有效长度 */
    char buffer[4068];
};
struct nm_rule_hdr { u32 flags; u32 uid; u16 v_len; u16 r_len; } /* 12 B */
struct nm_del_hdr  { u32 uid; u16 v_len; }                       /* 6 B */
~~~

命令（基线 NM_CMD_*）：UNSPEC=0  GET_VERSION=1  ADD_RULE=2  DEL_RULE=3  ADD_UID=4  DEL_UID=5  CLEAR_ALL=6  CLEAR_RULES=7  CLEAR_UIDS=8  GET_LIST=9  GET_UIDS=10。

标志：IS_DIR=1（内核置）、VIRTUAL_DIR=2（内核内部）、WHITEOUT=4（用户态置）；K2 新增 OPAQUE（第 13.3 节）。whiteout 时 r_len 被内核强制为 0。

批处理：ADD_RULE / DEL_RULE 的 buffer 是连续记录流，arg1 回写已消费偏移；上游逐条覆盖 status（缺陷，见 4.3），K2 改为保留首个错误。

---

### 4.3 上游 .replace 缺陷（实证，v2 移除 K1 的直接依据）

上游 metamount.sh 对 .replace 的处理是“白化父目录，再注入其子项”：

~~~
第一趟（whiteout）：目录含 trusted.overlay.opaque=y 或 .replace 标记 → rule add --whiteout <父目录>
第二趟（注入）   ：普通文件 / 符号链接 → rule add <vpath> <rpath>
~~~

该链路在内核中断开：

1. nm_alloc_rule（nomount.c:1269）中，whiteout 走 is_whiteout 分支：第 1279 行把 r_len 强制为 0；第 1293 行的真实路径探测被 (!is_whiteout) 短路，因此第 1297 行的 NM_FLAG_IS_DIR 永不被设置，第 1310 行也不会分配 this_dir。
2. 第二趟注入 /system/etc/foo 时，nomount_generate_virtual_topology（nomount.c:1152）找到父规则 /system/etc，第 1172-1173 行判定其非 IS_DIR → 返回 -ENOTDIR。
3. ADD_RULE 批处理循环（nomount.c:1475-1481）逐条覆盖 payload->status 且出错不中断，后续成功条目会覆盖 -ENOTDIR。

净效果：目录被白化（从命名空间消失），模块替代文件全部未注入，错误被静默吞掉。因此 NoMount 原生实现的 .replace 语义不成立。同理，HM 现有的 ensure_status 只看末条 status，也无法发现批量中间失败——这一缺陷在 K2 中一并修正（第 13.3 节）。

### 4.4 HM 现状（接入面）

- src/config.rs：enum Mode { Overlay, Magic, Ignore }，serde 小写，deny_unknown_fields。
- src/mount_tree.rs：共享 MountNode/MountSource；NodeFileType 含 RegularFile/Directory/Symlink/Whiteout；replace_for(backend) 判定 .replace；structural_sources 承载结构父链。
- src/plan/mod.rs：MountPlan { tree, overlay_ops, overlay_files, ... }；register() 做跨后端冲突检测；ensure_vfs_replace_supported() 当前对 VFS 的 .replace fail-fast。
- src/pipeline.rs：overlay → magic → KSU try-umount → 状态快照，失败事务式回滚。
- src/state.rs：RunState { overlay_modules, magic_modules, active_mounts, mode_stats }。
- 已落地分支 feat/nomount-vfs 含 src/vfs/ 约 1.4k 行与 K1 相关复杂度（本次修订将收敛）。

---

## 5. 架构总览

~~~
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
                 K2：HM 自有 VFS 内核
                 （fork NoMount，HM 独立 ABI / CLI / 分发）
                           │
                add_key("hybridmount")  ← HM 专属 key type
~~~

要点：

- **单一 Provider**：v2 不再探测多种实现，src/vfs/backend.rs 只绑定 K2。
- **自有 ABI**：key type 名、版本串、命令集与 flags 由 HM 定义（第 6 节）。
- **单向兼容**：K2 的 key type 与 nm CLI 不同，NoMount 元模块天然无法驱动 K2（需求即满足）。

---

## 6. K2 Wire 协议契约

### 6.1 key type 与身份

- K2 注册 HM 专属 key type，工作名 **hybridmount**（Phase 0 冻结，见第 17 节）。
- HM 用户态所有下发走 add_key("hybridmount", "trigger", &payload)。
- NoMount 的 nm CLI 只发 add_key("nomount", ...)：在只装 K2 的内核上得到 -ENODEV，payload->status 保持哨兵 -1 → nm 报错退出。**“K2 不兼容 NoMount 元模块”由此在 syscall 层物理成立，无需额外策略。**
- 反向：K2 不得注册 nomount key type，也不得依赖 NoMount 的 key type 存在。

### 6.2 payload 布局

- 基线沿用 NoMount v20 布局（magic / cmd / target_uid / status / arg1 / data_size / buffer[4068]），以降低 fork 的 rebase 成本并复用现有 src/vfs/protocol.rs。
- 因为不再需要对 K1 兼容，HM 可**增量**扩展：新增 flag 位、命令，或重定义保留语义（如 OPAQUE、失败下标复用 arg1）。扩展必须记录在协议版本号中（第 6.3 节）。
- magic 已改为 HM 专属值 0x4859425249444D4F（ASCII "HYBRIDMO"）：上游 nm CLI 不论是否检查版本串，都会在 preparse 校验 magic 时被 -EFAULT 拒绝。布局其余部分（字段顺序、偏移、命令号）仍是上游 v20 的。

### 6.3 版本握手与状态语义

- 启动先发 GET_VERSION；K2 返回 HM 自有版本串（工作值 "hm1"，Phase 0 冻结）。
- HM 维护“支持的协议版本集合”，初始为 {hm1}；不在集合内 → 拒绝该 Provider。
- **status 语义（K2 修正）**：
  - 成功：status == 0；
  - 失败：status == 首个错误的负 errno（不再被后续记录覆盖）；
  - 失败时 arg1 == 失败记录在 buffer 中的起始偏移；成功时 arg1 == 已消费字节数。
- HM 的 ensure_status / ensure_consumed 需同时校验 status 与 arg1 语义。
- 探测语义：K2 未加载时 add_key 返回 -ENODEV（key type 不存在），HM 据此判定“不可用”；不得把该错误与 K2 内部错误混淆。

### 6.4 路径语义

- 下发的 virtual / real 路径一律为绝对路径（HM 直接构造，不经 shell cwd）。
- 保留 PATH_MAX 限制；单条记录 sizeof(hdr) + v_len + r_len <= 4068。
- 分区提升沿用 planner 的 map_target 结果。
- 符号链接源：上游以 LOOKUP_FOLLOW 解析真实路径；K2 需明确“注入符号链接是否保留链接语义”并写入协议文档（第 17 节）。

---

## 7. 兼容矩阵与安全守卫

### 7.1 兼容矩阵（v2）

| 组合 | 支持 | 说明 |
| --- | --- | --- |
| HM 元模块 × K2 内核 | ✅ | 唯一支持的组合 |
| HM 元模块 × K1（NoMount 原生内核） | ❌ | 本版不支持；按配置降级 overlay / magic |
| NoMount 元模块 × K2 内核 | ❌ | key type 不同，nm CLI 得到 -ENODEV |
| NoMount 元模块 × K1 | ✅ | 与 HM 无关，属 NoMount 自身 |

### 7.2 运行期单向守卫

放弃双向互斥后，仍需防止 K2 与外来 NoMount **同时劫持**：

1. 加载/绑定 K2 前，HM 探测外来 NoMount：add_key("nomount", ...) 若得到有效响应 → 判定设备已集成 NoMount。
2. 命中则**不下发任何 VFS 规则、不加载 K2 LKM**；按 vfs_strict 决定是失败还是降级（第 12 节）。
3. 该探测只做一次，结果进入状态快照（vfs_foreign_nomount）。

说明：该守卫是单向的（不影响 NoMount 自身运行），且不读取对方规则、不做互斥仲裁。

### 7.3 内核集成层守卫（编译期）

- module/vfs/setup.sh 在 patch 内核前探测：已存在 CONFIG_NOMOUNT、fs/nomount/ 源码，或运行期 /proc/modules 已有 nomount 模块 → 拒绝集成并提示。
- 原因：K2 使用不同 key type，register_key_type 不会冲突，**内核不会自动阻止两者共存**；若同时活动将双重劫持 inode 操作向量，必须由集成脚本显式拦截。
- 反向：文档明确“同一内核树只集成一套”。

---

## 8. 数据模型变更

### 8.1 src/config.rs

- enum Mode 增加 Vfs（已落地），as_str() -> "vfs"。
- 新增可选严格开关 vfs_strict：true 时 VFS 不可用即启动失败，false 时降级（第 17 节未决）。
- 删除 v1 为 K1/K2 互斥引入的配置面（若有）。

### 8.2 src/plan/mod.rs

- MountPlan 增加 vfs_module_ids: Vec<ModuleId>（已落地）。
- 新增/保留 collect_vfs()（对齐 collect_magic()）。
- register() 将 Vfs 纳入同目标冲突判定（已落地）。
- ensure_vfs_replace_supported() 由 fail-fast 改为**生成 opaque 规则**（第 9.1 节）。

### 8.3 src/state.rs

- RunState 增加 vfs_modules、vfs_active_mounts、vfs_provider、vfs_foreign_nomount。
- ModeStats 增加 vfs。
- 语义：active_mounts 只统计真实挂载（overlay + magic）；VFS 是注入，不计入。

### 8.4 src/vfs/backend.rs 收敛

- 删除 VfsProvider::Nomount 分支与双 key type 探测；只保留 HM provider。
- 删除 Error::VfsProviderConflict 的双向仲裁语义（保留外来 NoMount 检测的错误/降级路径）。
- VfsKernel trait 保留（用于测试注入 mock），但生产实现只有 KeyringKernel("hybridmount")。

---

## 9. 规则映射（src/vfs/rule.rs，纯函数）

输入：MountTree 中标注为 Vfs 的来源；输出：Vec<VfsRule>。

| 节点类型 | 映射 |
| --- | --- |
| RegularFile | injection：vpath -> rpath |
| Symlink | injection（语义待第 17 节确认） |
| Whiteout | whiteout vpath |
| Directory | 只作为遍历结构，不产生规则；普通合并由内核虚拟拓扑完成 |
| Directory 且 replace | **opaque 目录规则**：保持目录可见，隐藏真实条目，仅显示注入子项 |

- 目标路径取 planner 的 map_target 结果（含分区提升）。
- 一个目标的多模块层：每个目标最多下发一条规则；node.sources 中最后一个 backend == vfs 且非目录的来源（模块顺序靠后）获胜（v1 已决，保留）。
- 目录结构父链（structural_sources）不产生 whiteout 或注入。
- opaque 目录同时**独占其后代**：其后代不得由 overlay/magic 承接（第 10 节）。

### 9.1 .replace = opaque 目录

- 语义对齐 OverlayFS 的 trusted.overlay.opaque=y：目录存在、真实条目全部隐藏、只显示模块注入的子项。
- 实现：K2 新增用户态可置的 OPAQUE 标志（第 13.3 节），配 IS_DIR 使用，内核据此建立仅含注入子项的目录节点（复用上游 VIRTUAL_DIR 的迭代/查找语义，但由我方显式定义与校验）。
- v1 的 userspace VIRTUAL_DIR hack 作废：K2 是自有内核，直接提供一等公民语义，不再需要依赖上游内部标志。
- 与上游 whiteout 的区别：whiteout 让目录“消失”，opaque 让目录“留壳换内容”。
- 用户态映射：planner 对 Vfs 的 replace 目录下发一条 opaque 规则；其子项仍按普通注入下发，父目录已具 IS_DIR 语义，不再触发 -ENOTDIR。

---

## 10. 规划不变量与冲突

沿用现有不变量：

- 同一文件路径只能进入一个后端；
- 普通目录可被多个后端作为结构节点共享；
- 文件 / 类型 / .replace 冲突在 plan 阶段显式报错，executor 不裁决。

VFS 新增：

1. **同目标唯一**：Vfs 与 overlay/magic 命同一目标 → Error::PlanConflict。
2. **遮蔽约束**：
   - VFS 目标不得存在被 Overlay / Magic 挂载的祖先目录；
   - 被 Overlay / Magic 挂载的目录不得包含 VFS 后代；
   - 违反 → Error::PlanConflict（附加 shadowed_by 说明）。
   原因：VFS 规则作用在真实目录上，任何祖先挂载点都会遮蔽其后代注入。
3. **opaque 独占**：Vfs 的 replace 目录一旦标记 opaque，其整个子树只能由 Vfs 承接；若存在 overlay/magic 后代 → Error::PlanConflict。
4. 若设备存在外来 NoMount（第 7.2 节）→ VFS 不可执行，按 vfs_strict 失败或降级。

---

## 11. 执行顺序与回滚（src/pipeline.rs）

- 阶段顺序：overlay → magic → vfs。VFS 最后执行；遮蔽冲突已在 plan 阶段排除。
- 回滚：每个成功操作注册进现有 Transaction：
  - **下发前先构建并登记本次完整批次**；失败时按该批次的虚拟路径逐条 DEL_RULE（容忍 ENOENT：规则本就不存在时内核回写 -ENOENT，不是回滚失败）。apply_rules 非原子，中途失败时已生效的前缀同样必须删除；
  - **不使用 CLEAR_RULES**：该命令会清空 Provider 的整张规则表，可能影响同 Provider 上其它来源的规则；
  - opaque 目录规则与其子项规则同批登记、同批回滚；
  - 依赖 K2 的“首个错误 + 失败下标”语义精确定位并清理已生效前缀。
- Boot 安全：K2 加载与规则下发前写 /data/adb/hybrid-mount/vfs_boot_guard，成功后删除；遗留标记表示上次启动失败，下次跳过 VFS 后端并保留失败快照。

---

## 12. 配置 / CLI / WebUI 契约

- config.toml：default_mode 与 [rules.*.paths] 接受 "vfs"；未知字段仍拒绝。
- show-config / save-config：自动支持新枚举值，无需新命令。
- status：新增 vfs_modules、vfs_active_mounts、vfs_provider、vfs_foreign_nomount、vfs_unavailable（可选）。
- mode_stats：新增 vfs。
- WebUI：模式选择项增加 “VFS”；状态页展示 Provider 与 VFS 注入计数。
- 兼容：旧配置不含 vfs，行为不变；新字段均可选，升级不破坏。

### 12.1 K2 独有 CLI（hybrid-mount vfs ...）

作为 K2 的官方控制面，与 nm CLI 明确区分：

| 子命令 | 作用 |
| --- | --- |
| hybrid-mount vfs status | 打印 key type、协议版本、规则数、UID 数、外来 NoMount 检测结果 |
| hybrid-mount vfs list | 列出当前规则（GET_LIST） |
| hybrid-mount vfs uid list / add / del | UID 隔离管理 |
| hybrid-mount vfs rules clear | 清空规则（需显式确认参数） |
| hybrid-mount vfs doctor | 自检：key type 可注册、GET_VERSION、opaque 支持、批量错误回传 |

- 输出复用 HM 既有 JSON 约定；诊断日志走 stderr，不污染 stdout。
- 这些命令只用于调试/诊断；正常挂载路径仍由 pipeline 驱动。

---

## 13. K2 内核子系统

### 13.1 身份与 ABI

- **来源**：fork NoMount 内核源码，在 HM 仓库内独立演进。
- **位置**：module/vfs/src/（fs/hybridmount/ 等），License **GPL-3.0-only**，与现有 module/lkm/（GPL-2.0-only，ext4 nuke）分属两个许可子树，禁止合并。
- **ABI**：注册 HM 专属 key type（工作名 hybridmount）；payload 布局以 NoMount v20 为基线，版本串由 HM 自控；与 NoMount 元模块互不兼容。
- **上游同步**：记录所 fork 的 NoMount commit，便于 rebase 与差异追踪。

### 13.2 分发与构建

- built-in：提供 module/vfs/setup.sh 自动 patch（含第 7.3 节互斥守卫）。
- LKM：module/vfs/binaries/hybridmount-<gki>-<kernel>.ko + list.txt SHA256 校验，复用 HM 现有“精确候选 + 校验 + boot 熔断”机制。
- 构建：xtask 增加内核模块构建矩阵（kernel line × Android/GKI × arch），产物进入发布 ZIP。
- 覆盖范围：见第 17 节未决（先覆盖 GKI 5.10+ 主流，还是对齐上游全量）。

### 13.3 相对上游的优化清单（K2 的存在理由）

以下四项均有代码级实证，构成 K2 相对 NoMount 的实质增量：

1. **opaque 目录一等公民**
   - 上游：.replace 只能表达为目录 whiteout；因 whiteout 不带 IS_DIR，其子项注入返回 -ENOTDIR（第 4.3 节）。
   - K2：新增用户态可置的 OPAQUE 标志，显式定义“留壳换内容”语义并校验 v_len/类型；.replace 由此获得与 OverlayFS 对齐的正确行为。

2. **批量错误不丢失**
   - 上游：ADD_RULE 循环逐条覆盖 status 且出错不 break（nomount.c:1475-1481），批量中间失败被后续成功掩盖；HM 的 ensure_status 只看末条，同样漏检。
   - K2：保留**首个错误**的 errno，并把失败记录下标回写 arg1；HM 据此精确定位并回滚。

3. **UID 隔离查询优化**
   - 上游：nomount_is_uid_blocked 对 nomount_uids 做 O(n) 线性扫描，且位于每条 lookup 的最前置；被隔离 UID 越多，每次 dentry 查找越慢（nomount.c:188/271/317/353）。
   - K2：改用有序数组二分 / 位图 / 哈希结构，并把判定与 bloom 快路径合并——bloom 未命中时无需扫描（bloom 未命中即无规则，dentry-drop 逻辑也不会触发，语义等价）。

4. **可观测性与诊断**
   - 上游：无统计接口，仅 GET_LIST / GET_UIDS。
   - K2：新增诊断命令（规则数、UID 数、opaque 数、批量错误计数），由 hybrid-mount vfs status / doctor 消费。

（可选，Phase 3 评估）**批量原子提交**：整批先校验后插入，避免半生效；HM 已在用户态做批次登记与回滚，内核侧原子化可进一步降低失败窗口，但需评估锁开销。

### 13.4 协议扩展清单（相对 v20）

| 扩展 | 形式 | 用途 |
| --- | --- | --- |
| OPAQUE 标志 | 新增 flag 位，与 IS_DIR 组合 | .replace 语义 |
| 失败下标 | 失败时 arg1 = 失败记录偏移 | 精确回滚与诊断 |
| 诊断命令 | 新增 K2_CMD_GET_STATS 等 | hybrid-mount vfs status / doctor |
| 版本串 | "hm1"（自有命名空间） | 拒绝非 K2 Provider |

扩展以增量方式加入，并在 GET_VERSION 的版本串中体现兼容下限。

---

## 14. 归属 / co-author

- 派生文件头保留 NoMount 版权与 SPDX-License-Identifier: GPL-3.0-only，并注明来源。
- 集成提交加 Co-authored-by: maxsteeel <109047395+maxsteeel@users.noreply.github.com> trailer。
- 内核模块保留 MODULE_AUTHOR("maxsteeel")，必要时追加 HM 维护者。
- module.prop 的 author 与 README 的 Special Thanks 收录 NoMount / maxsteeel。
- 许可兼容性：NoMount GPL-3.0 与 HM 核心 GPL-3.0-only 兼容；不得并入 GPL-2.0-only 的 module/lkm/ 子树。
- v2 不再主张与上游 ABI 兼容，故无需协商兼容窗口；但 fork 关系与版权归属不变。

---

## 15. 测试与验证

### 15.1 主机侧（无需设备）

- tree → VfsRule 纯函数单测：文件 / 符号链接 / whiteout / 目录结构 / opaque(.replace) / 分区提升。
- 协议编解码单测：payload、rule_hdr、del_hdr 的字节级布局与批处理游标；opaque 标志位。
- 状态语义单测：首个错误保留 + 失败下标回传（构造“中间失败、末条成功”的假 payload）。
- Provider 守卫单测：外来 NoMount 命中 → 不加载 K2、不下发规则；未命中 → 正常绑定。
- 冲突与遮蔽单测：同目标多后端、祖先挂载遮蔽、opaque 独占子树。
- 现有门禁：cargo fmt、clippy -D warnings、cargo test --workspace、禁用符号检查。

### 15.2 内核侧

- 各 kernel line 的 K2 编译通过；
- 实机 ABI 校验：key type 注册、GET_VERSION、opaque、批量错误回传、UID 隔离。

### 15.3 实机矩阵

| 场景 | 验证点 |
| --- | --- |
| K2（HM 自有 LKM） | 握手、注入、whiteout、opaque(.replace)、UID、/proc/mounts 干净 |
| K2（built-in） | 同上，且不加载 LKM |
| NoMount 元模块 × K2 | nm CLI 得到 -ENODEV、无法驱动 K2 |
| 外来 NoMount 存在 | HM 不下发规则、不加载 K2、按 vfs_strict 降级或失败 |
| 降级 | 无 K2 时按配置回落 overlay/magic，状态正确 |
| 遮蔽冲突 | VFS 祖先被挂载用例在 plan 阶段报错 |
| 批量回滚 | 注入中间失败时已生效前缀被删除、失败下标正确 |

---

## 16. 风险

| 风险 | 缓解 |
| --- | --- |
| K2 内核维护成本（无 K1 兜底） | LKM 构建矩阵 + built-in patch；先覆盖主流 GKI |
| 自有 ABI 与上游分叉后难以借鉴 | payload 基线沿用 v20，扩展走增量；记录 fork commit |
| 内核 ABI 不稳定 | 版本握手 + 支持集合；K2 由 HM 自控版本 |
| opaque 目录实现与上游 VIRTUAL_DIR 语义漂移 | 显式定义 + 实机验证 + 单测覆盖 |
| 双重劫持（K2 与外来 NoMount 并存） | 第 7.2/7.3 节单向守卫 |
| 遮蔽顺序 | plan 阶段强制不变量 |
| 状态语义混淆 | VFS 与挂载分字段统计 |
| K2 引入启动循环 | 复用 HM boot 熔断 + 严格回滚 |
| S_PRIVATE / xattr 代理副作用 | 实机观察，必要时在上游反馈 |

---

## 17. 未决问题

1. **HM key type 名与版本串**：工作名 hybridmount / "hm1" 待 Phase 0 冻结；需确认不触发意外 request_module、不与常见 key type 冲突。
2. ~~**magic 是否改为 HM 专属值**~~ **已决（v2）**：改为 0x4859425249444D4F（ASCII "HYBRIDMO" 的大端读数）。上游常量为公开值，沿用只能靠 key type 名隔离；换掉后旧 nm CLI 在 preparse 阶段即被 -EFAULT 拒绝，二进制层面彻底断开。该常量同时是 `full_name_hash` 种子，用户态 `src/vfs/protocol.rs::MAGIC` 与内核 `HYBRIDMOUNT_MAGIC_SIG` 必须逐字节一致，由 `wire_magic_is_pinned_to_the_hm_value` 测试钉死。注意换 magic 后**旧版预编译 .ko 与新用户态不兼容**，需重跑 DDK 工作流刷新 `module/vfs/binaries/`；不匹配时表现为探测失败并降级，不会导致启动失败。
3. **符号链接源语义**：K2 注入符号链接是否保留链接语义（上游以 LOOKUP_FOLLOW 解析真实路径），需实机确认并写入协议文档。
4. **K2 覆盖矩阵**：先覆盖 GKI 5.10+ 主流版本，还是对齐上游全量。
5. **vfs_strict 默认值**：默认 false（降级）还是 true（失败）需产品决策。
6. **外来 NoMount 探测的可靠性**：add_key("nomount") 的 -ENODEV 判定是否存在 request_module 副作用或误判；Phase 0 实机确认。
7. ~~K1 与 K2 的 wire 层区分~~ **已消解（v2）**：放弃 K1 后不存在该问题。
8. ~~同目标多模块 VFS 规则~~ **已决（v1）**：同目标只下发一条规则，最后一个非目录 Vfs 来源获胜。

---

## 18. 分期计划（本文档不包含实现）

- **Phase 0**：冻结 K2 key type 名、版本串、magic、OPAQUE 标志位；记录 fork commit；实机确认第 17.3 / 17.6 项。
- **Phase 1**：src/vfs/rule.rs 纯函数 + opaque 映射 + 单测（不接触设备）。
- **Phase 2**：src/vfs/protocol.rs 字节级协议（含失败下标语义）+ backend.rs 收敛为单一 Provider + 外来 NoMount 守卫（mock 测试）。
- **Phase 3**：K2 内核 fork 子树（含 opaque、错误保留、UID 优化）+ setup.sh + LKM 构建矩阵 + boot 熔断。
- **Phase 4**：pipeline 接入、回滚、状态字段、hybrid-mount vfs CLI、WebUI、打包与文档。
- **Phase 5**：清理 v1 遗留的 K1 代码与配置面。

---

## 19. 参考

- NoMount：https://github.com/maxsteeel/nomount
- NoMount 内核集成：kernel/README.md（built-in / LKM / setup.sh）
- HM 架构契约：docs/ARCHITECTURE.md
- HM 挂载树：src/mount_tree.rs；规划器：src/plan/mod.rs；流水线：src/pipeline.rs
