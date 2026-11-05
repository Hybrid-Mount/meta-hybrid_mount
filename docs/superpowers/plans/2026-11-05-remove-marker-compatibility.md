# Remove Marker Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 兼容目录 .remove 标记，三后端统一隐藏目标目录，不删除原文件。

**Architecture:** 扫描阶段将目录标记规范化为共享树里的 whiteout，并停止收集其后代。规划继续匹配原逻辑路径；既有分区映射和受保护挂载边界决定请求能否执行。

**Tech Stack:** Rust、OverlayFS/Magic Mount/VFS、现有单元测试及 Linux 隔离挂载测试。

**Spec:** [上游选择性吸收设计](../specs/2026-11-05-upstream-improvements-design.md)

## Global Constraints

- 在分区映射 P1–P5 完成后实施；使用同一布局和 protected_targets，不新建后端专用映射。
- .remove 优先于 .replace；精确标记文件本身不挂载，普通名字 ordinary.remove 不受影响。
- 不删除磁盘内容，不将模块根 remove 混为目录 .remove；不改 hm1 或 NoMount wire ABI。
- 禁止删除 system/分区/extra_mount 根；跨子挂载不能等价隐藏时跳过请求并报告。
- 路径规则 ignore 能关闭删除；激活的父 .remove 不接受子规则重新注入；跨模块冲突显式报错。

## Review Focus

- 标记与普通同名后缀文件：R1 精确匹配。
- 符号链接伪装标记或断链目标：R1 校验标记自身类型，R2 验证目标不跟随删除。
- .replace 与 .remove 同时存在：R1 保留删除优先级和零后代贡献。
- 跨模块目标/后代贡献：R2 在执行前报告冲突，不能依赖扫描顺序。
- 分区根或有子挂载的目标：R2 拒绝不安全请求，其余正常分支仍可执行。

### Task 1: R1 — 扫描与规范化

**Files:** Modify `src/defs.rs`, `src/scanner.rs`, `src/mount_tree.rs`（如需保存来源）; Test `src/scanner.rs` 的现有内联测试。

**Interfaces:** `REMOVE_DIR_FILE_NAME: &str = ".remove"`；`ModuleEntry` 增加 `directory_remove: bool`（默认 false，所有构造点同步）；对有效标记的目录输出 `file_type = Whiteout, replace = false, directory_remove = true`，relative/source 保持目录本身，不收集后代。目录根请求保留诊断证据供 R2 拒绝，不悄悄丢弃。

- [ ] 写 `remove_marker_overrides_replace`、`remove_marker_does_not_mount_children`、`ordinary_remove_suffix_is_regular_file`、`symlink_remove_marker_is_invalid`。断言 Foo/.remove 和 Foo/.replace 共存时只输出 Foo 删除贡献；`ordinary.remove` 仍为普通文件。
- [ ] `cargo test --workspace scanner`，确认旧扫描器不能满足新断言。
- [ ] 用 symlink_metadata 验证标记自身为普通文件，精确识别，停止后代扫描；无效标记保留普通目录语义并记录无效标记诊断，绝不将标记文件当内容挂出。保持真正 char-device whiteout 原义。
- [ ] 重跑 scanner/plan 测试，覆盖 extra_mount、空目录标记、模块根 remove（无点号）及禁用模块。
- [ ] 独立提交扫描兼容，不绕过 R2 直接开放未验证的删除执行路径；必要时同一交付提交包含 R2 才启用该功能。

### Task 2: R2 — 三后端删除语义与边界

**Files:** Modify `src/plan/mod.rs`, `src/pipeline.rs`, `src/magic_mount/exec.rs`, `src/vfs/rule.rs`, `src/vfs/protocol_tests.rs`, `src/state.rs`; Extend `tests/linux/partition_mount_boundaries.sh`; Update `THIRD_PARTY.md`, `docs/README_ZH.md`, `docs/ARCHITECTURE.md`。

**Interfaces:** 消费 R1 的 directory_remove 和分区计划的 protected_targets/operation_errors；使用现有 `VfsAction::Whiteout`，不把隐藏目录误编码成 opaque。规划诊断复用 P4 的 OperationDiagnostic，在 code 中区分 `partition_root_remove` / `protected_submount_remove`，不将单个删除请求失败误标为整个分区不可用。

- [ ] 写 `directory_remove_hides_target_for_all_backends`、`ignored_remove_leaves_stock_visible`、`remove_conflicts_with_other_module_descendant`、`partition_root_remove_is_rejected`、`remove_over_child_mount_is_not_partially_applied`。断言被删目录在合并视图不存在、原磁盘内容仍可在独立原生视图读取；父删除下无注入规则。
- [ ] `cargo test --workspace`，证明三个后端现有目录/whiteout 分支在新场景中的缺口，已支持的编码场景直接保留 PASS。
- [ ] 将有效删除通过现有 whiteout 机制实现。OverlayFS 需要父层 whiteout、Magic 需要父镜像省略、VFS 需要 whiteout 规则；不能通过“把 Foo 挂空目录”代替“Foo 不存在”。分区根/保护边界拒绝要出诊断且不能生成任何删除副作用。
- [ ] 验证与 .replace、普通文件 whiteout、后端冲突、NoMount 20/21/22 编码和热操作所有权的回归。运行 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 和 `sh tests/linux/partition_mount_boundaries.sh`；需要的环境未就绪时明确保留设备/挂载验证未完成。
- [ ] 文档注明标记用法、优先级、禁止根删除和错误展示，保留上游提交归属，独立提交。若意外需要改内核 ABI，停止本任务并补设计，不扩展既定范围。

## 完成条件

扫描器新标记与执行器作为同一功能交付；不能发布“已识别 .remove 但没有可靠隐藏目录”的中间状态。
原文件未删除、拒绝目标零副作用、跨模块冲突确定、三后端视图一致，才可称兼容支持完成。
