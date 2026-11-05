# Partition Path Mapping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 按实际挂载布局统一现有分区路径，并保护子挂载；冲突分区跳过报错。

**Architecture:** 只读发现生成不可变布局，扫描与规划消费同一对象，执行器只执行过滤后的共享树。启动保存全部名单分区的基线；热操作比较相关布局，卸载使用已记录身份。

**Tech Stack:** Rust、procfs/rustix、Linux mount namespace、Vue/TypeScript、Vitest。

**Spec:** [分区映射设计](../specs/2026-11-05-partition-path-mapping-design.md)、[上游经验](../specs/2026-11-05-upstream-improvements-design.md)

## Global Constraints

- 不扩展现有名单或 extra_mount 准入，不修改模块源目录。
- 两个候选不是同一可见树时，整个分区跳过报错，正常分区可提交；全局挂载表读取失败在执行前停止。
- 原模块路径匹配规则，最终路径检测冲突；三后端使用同一映射。
- 祖先挂载、.replace、whiteout 不能绕过拒绝分区；不能安全保留语义时跳过相应操作并报告。
- 局部分区错误不触发整轮回滚，状态明确 degraded；卸载不依赖重新解析布局。

## Review Focus

- 同设备不同 bind root 或同根不同子挂载：P1/P2 必须拒绝等价推断。
- 根路径不存在却有 system 内目录：P2/P3 不漏扫模块顶层布局。
- .replace 跨过拒绝分区或子挂载：P4 不允许通过拆分改变替换语义。
- 热 reload 预检失败：P5 保留原规则，首次加载也比较启动基线。
- 部分挂载但 UI 显示完全成功：P5 区分确认效果与分区错误，旧字段兼容。

### Task 1: P1 — 挂载与路径证据模型

**Files:** Modify `src/sys/mountinfo.rs`, `src/main.rs`（注册模块）; Create `src/partitions.rs`, `src/partitions/tests.rs`。

**Interfaces:** `MountEntry` 增加 `root: PathBuf`, `parent_id: i32`；`PathEvidence { canonical: PathBuf, dev: u64, ino: u64, visible_mount_id: i32 }`；`PathProbeFailure` 区分 Missing、Unreadable、SymlinkLoop、DisallowedTarget；`LayoutSnapshot { mounts: Vec<MountEntry>, paths: BTreeMap<PathBuf, PathEvidence>, path_errors: BTreeMap<PathBuf, PathProbeFailure> }`；`capture_layout_snapshot(names: &[String]) -> Result<LayoutSnapshot>`。Linux探测与纯决策分离；其他平台测试使用显式夹具。

- [ ] 增加 `snapshot_preserves_root_parent_and_visible_mount`、`stacked_mounts_use_visible_id_not_max_id`，断言解析完整 mountinfo 字段且选择路径实际可见挂载，覆盖文件子挂载和转义空格路径。
- [ ] `cargo test --workspace partitions`：新接口尚未实现时应 FAIL；修正前不得把第一条/最大 id 当可见性答案。
- [ ] 实现快照采集，优先使用系统可提供的路径挂载身份；不支持时只能用经过验证的拓扑推导，仍不确定则记录该分区证据错误。全局读取错误返回 Err。
- [ ] 重跑夹具及 `cargo test --workspace mountinfo`，确认原有确认/回滚消费者无回归。
- [ ] 独立提交证据模型与测试。

### Task 2: P2 — 分区布局决策

**Files:** Modify `src/partitions.rs`, `src/partitions/tests.rs`; Read `src/defs.rs`, `src/config.rs`。

**Interfaces:** `PartitionLayout { roots: BTreeMap<String, PathBuf>, identities: BTreeMap<String, PartitionIdentity>, errors: BTreeMap<String, PartitionDiagnostic> }`；`PartitionIdentity` 保存 canonical、根 dev/ino 及后代相对路径的可见身份，不以 mount id 作稳定指纹；`PartitionDiagnostic { partition: String, code: String, reason: String, modules: Vec<String> }`；`discover_layout(snapshot: &LayoutSnapshot, managed: &[String], extras: &[String]) -> PartitionLayout`；`PartitionLayout::map_target(&self, relative: &str) -> Result<(String, PathBuf)>`。统一 serde 默认值；错误为每分区结果，不因单分区错误返回全局 Err。

- [ ] 按设计决策表逐项写夹具，正向 vendor 目标断言 `/vendor/etc/demo`，反向断言 `/system/vendor/etc/demo`；同时覆盖 odm、断链、循环、不允许目标、缺失和仅普通顶层目录。
- [ ] `cargo test --workspace partitions`，确认尚未实现的决策 FAIL。
- [ ] 实现两入口映射、bind 根身份与后代可见拓扑比较；证据不足与不同树均拒绝。system 正常路径保留；extra_mount 只沿显式入口。
- [ ] 跑完整 partitions 测试，确保不同 root 的同设备夹具失败于合并而非被接受；符号链接解析到同一路径不重复比较。
- [ ] 独立提交纯布局决策。

### Task 3: P3 — 只读扫描和统一规划

**Files:** Modify `src/scanner.rs`, `src/config.rs`, `src/plan/mod.rs`, `src/mount_tree.rs`, `src/pipeline.rs`, `src/runtime/policy.rs`, `src/runtime/hot.rs`, `src/state.rs`。

**Interfaces:** `PlanInput::promoted_partitions` 替换为 `layout: &'a PartitionLayout`；`MountPlan` 增加 `partition_errors: Vec<PartitionDiagnostic>`。`ModuleEntry` 增加用于已确认模块别名的可选链接目标证据，保持 source_path/relative 原值；保留 `list_modules` 的只读契约。所有 build_plan 调用在同一提交迁移，禁止留下空提升集合替代布局。

- [ ] 添加 `top_level_vendor_maps_to_system_subdir`、`rejected_partition_is_filtered_for_all_backends`、`mapped_aliases_do_not_bypass_conflicts`、`non_alias_symlink_is_preserved`；断言原规则选择不变，非 ignore 的 vendor 请求有诊断，禁用/黑名单/ignore 不产生假错误。
- [ ] `cargo test --workspace`，观察上述新行为在旧规划器下 FAIL；已知无关 xattr CI 失败单独记录，不能修改断言绕过。
- [ ] 将扫描入口改为逻辑名单而非单纯 /<分区> 存在；真实目录只扫描一次；过滤确认的布局别名，映射后做冲突检查，并在所有前端接入布局对象。
- [ ] 重跑 scanner/plan 的全量测试，检查整模块 Overlay 拆分、空根 .replace、混合后端冲突及原生 whiteout 行为。
- [ ] 独立提交规划集成与所有调用点迁移。

### Task 4: P4 — 祖先与子挂载保护（吸收 Mountify 思路）

**Files:** Modify `src/plan/mod.rs`, `src/pipeline.rs`, `src/overlayfs/overlayfs.rs`, `src/magic_mount/exec.rs`; Create `tests/linux/partition_mount_boundaries.sh`；必要的新测试注册在现有 workflow。

**Interfaces:** `MountPlan` 增加 `protected_targets: Vec<PathBuf>` 和 `operation_errors: Vec<OperationDiagnostic>`；`OperationDiagnostic { module_id: String, logical_path: String, target: Option<PathBuf>, code: String, reason: String }`。受保护目标由拒绝分区的所有候选根及实际子挂载边界生成。新增纯函数 `crosses_protected_target(target: &Path, protected: &[PathBuf]) -> bool` 使用路径组件边界；执行路径消费过滤后的 tree 和同一快照，不重新从原模块整树构建 lowerdir。

- [ ] 增加 `parent_overlay_cannot_restore_rejected_vendor`、`etc_overlay_preserves_permissions_submount`、`new_file_beside_submount_is_kept`、`replace_crossing_protected_target_is_rejected`，断言计划不覆盖保护根、不丢新增文件且错误可追溯。
- [ ] `cargo test --workspace` 定向运行新测试，确认旧“父挂载后尽力恢复”不能满足这些边界断言。
- [ ] 在应用父挂载前拆分合并型操作；对不能等价拆分的 .replace/删除/目录替换请求局部跳过报错。Magic 与 Overlay staging 都只消费过滤后贡献；保留原模块 symlink 和 whiteout 语义。
- [ ] 在可销毁 Linux mount namespace 运行 `sh tests/linux/partition_mount_boundaries.sh`，验证父/子 bind、嵌套与文件子挂载、执行失败后的恢复、模块新增项和原生文件保留；无环境时不可将夹具当作实际挂载验证。
- [ ] 独立提交边界保护与测试，更新 `docs/RUNTIME.md` 的局部降级说明。

### Task 5: P5 — 基线、热操作与可见诊断

**Files:** Modify `src/runtime/boot.rs`, `src/runtime/ledger.rs`, `src/runtime/hot.rs`, `src/state.rs`, `src/pipeline.rs`, `webui/src/lib/types.ts`, `webui/src/lib/api.ts`, `webui/src/lib/api.test.ts`, `webui/src/ui/md3/pages/modules.vue`, `webui/src/ui/miuix/pages/modules.vue`, `webui/src/ui/md3/pages/status.vue`, `webui/src/ui/miuix/pages/status.vue`，按现有国际化结构补充文案。

**Interfaces:** ledger 增加 `partition_layout: Option<PartitionLayout>`，保存所有名单结果；RunState 增加 `degraded: bool`、`partition_errors: Vec<PartitionDiagnostic>`、`operation_errors: Vec<OperationDiagnostic>`；AppModule 增加同名诊断字段，均带 serde 默认。hot load 响应增加 degraded/errors，保留现有 ok/generation；query 不改 ledger。初始化流程复用应用前快照。只有经过 ledger 所有权身份验证的自有覆盖可解释为相对启动基线的预期变化。

- [ ] 添加 `partial_partition_failure_keeps_confirmed_ownership`、`first_hot_load_checks_boot_layout`、`owned_overlay_is_expected_layout_change`、`unowned_overlay_cannot_bypass_layout_guard`、`reload_layout_failure_preserves_before_rules`、`legacy_ledger_allows_unload_only`、`partition_error_does_not_set_suggest_ignore`；兼容旧 JSON 的字段缺省测试必须通过。
- [ ] 运行 `cargo test --workspace` 与 webui 目录下 `pnpm test`，确认新状态和展示测试在旧实现下 FAIL。
- [ ] 接入应用前基线、相关布局比较及 reload 预检；卸载按保存身份执行。提交有效分区并记录 degraded；布局全局读取失败提前终止，不在已部分提交后返回触发回滚的总错误。
- [ ] UI 同时展示“部分挂载”和原因；全被跳过不显示已挂载。运行 Rust 全量测试、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`；webui 目录执行 `pnpm lint`、`pnpm test`、`pnpm build`。
- [ ] 更新 `docs/ARCHITECTURE.md`、`docs/RUNTIME.md`、中文使用文档，独立提交；记录 Android 四类布局的真机验证状态，不自动发布或刷机。
